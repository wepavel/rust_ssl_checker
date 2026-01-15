use super::notifiers::BaseNotifierTrait;
use super::sources::DomainSourceTrait;
use addr::parse_domain_name;
use base::prelude::{
    anyhow::{anyhow, Result},
    chrono::{self, DateTime, NaiveDateTime, Utc},
    once_cell::sync::Lazy,
    serde_json::{self, json},
    tokio::{self, net::TcpStream},
    tracing,
};
use futures::future::join_all;
use std::collections::{HashMap, HashSet};
use whois_rust::{WhoIs, WhoIsLookupOptions};

pub struct DomainCheckerService {
    sources: Vec<Box<dyn DomainSourceTrait>>,
    notifiers: Vec<Box<dyn BaseNotifierTrait>>,
    ssl_alarm_days: i64,
    alarm_days: i64,
    dcl: &'static str,
}

impl DomainCheckerService {
    const EXPECTED_ERRORS: &'static [&'static str] = &[
        "timed out",
        "Connection timed out",
        "Connection refused",
        "tlsv1 unrecognized name",
        "tlsv1 alert internal error",
        "Name has no usable address",
        "failed to lookup address",
        "Host is unreachable",
    ];
    const WHOIS_CLIENT: Lazy<WhoIs> = Lazy::new(|| {
        WhoIs::from_string(Self::SERVERS_JSON)
            .expect("Не удалось загрузить servers.json из include_str!")
    });
    const SERVERS_JSON: &str = include_str!("../../../servers.json");
    const TXT_PATTERNS: &'static [&'static str] =
        &["_dmarc", "_domainkey", "_acme-challenge", "_spf"];
    pub fn new(
        sources: Vec<Box<dyn DomainSourceTrait>>,
        notifiers: Vec<Box<dyn BaseNotifierTrait>>,
        ssl_alarm_days: i64,
        alarm_days: i64,
    ) -> Self {
        Self {
            sources,
            notifiers,
            ssl_alarm_days,
            alarm_days,
            dcl: "DomainCheckerService",
        }
    }

    fn to_root_domain(&self, domain: &str) -> Option<String> {
        let mut d = domain.trim().to_lowercase();

        if d.starts_with("*.") {
            d = d[2..].to_string();
        }

        let parsed = parse_domain_name(&d).ok()?;
        let root = parsed.root()?;
        let parts: Vec<&str> = root.split('.').collect();

        if parts.len() >= 2 {
            Some(format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]))
        } else {
            Some(root.to_string())
        }
    }

    fn filter_domain(&self, domain: &str) -> Option<String> {
        let mut d = domain.trim().to_lowercase();

        if d.starts_with("*.") {
            d = format!("test.{}", &d[2..]);
        }

        let labels: Vec<&str> = d.split('.').collect();

        if labels.iter().any(|lbl| Self::TXT_PATTERNS.contains(lbl)) {
            return None;
        }

        if labels.len() < 2 {
            return None;
        }

        Some(d)
    }

    async fn check_ssl_expiry(hostname: &str) -> Result<(DateTime<Utc>, String, String)> {
        let hostname_idn = idna::domain_to_ascii(hostname)
            .map_err(|e| anyhow!("IDN conversion failed: {}", e))?;

        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            TcpStream::connect(format!("{}:443", hostname_idn)),
        )
        .await
        .map_err(|_| anyhow!("Connection timed out"))??;

        let connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()?;

        let connector = tokio_native_tls::TlsConnector::from(connector);
        let tls_stream = connector.connect(&hostname_idn, stream).await?;

        let cert = tls_stream
            .get_ref()
            .peer_certificate()?
            .ok_or_else(|| anyhow!("No certificate found"))?;

        let der = cert.to_der()?;
        let (_, cert_parsed) = x509_parser::parse_x509_certificate(&der)
            .map_err(|e| anyhow!("Certificate parse error: {}", e))?;

        let expiry = cert_parsed.validity().not_after;
        let expiry_datetime = DateTime::from_timestamp(expiry.timestamp(), 0)
            .ok_or_else(|| anyhow!("Invalid timestamp"))?;

        let serial = format!("{:X}", cert_parsed.serial);

        let issuer = cert_parsed
            .issuer()
            .iter_organization()
            .next()
            .and_then(|cn| cn.as_str().ok())
            .unwrap_or("Unknown")
            .to_string();

        Ok((expiry_datetime, serial, issuer))
    }

    async fn check_domain_expiration(hostname: &str) -> Result<DateTime<Utc>> {
        let options = WhoIsLookupOptions::from_string(hostname)?;
        let lookup_result = Self::WHOIS_CLIENT.lookup_async(options).await?;

        Self::parse_whois_expiry(&lookup_result)
    }

    fn parse_whois_expiry(whois_text: &str) -> Result<DateTime<Utc>> {
        let expiry_patterns = vec![
            "paid-till:",
            "registry expiry date:",
            "expiry date:",
            "registrar registration expiration date:",
            "expiration date:",
            "expires:",
            "expire:",
            "expiration time:",
        ];

        for line in whois_text.lines() {
            let line_trimmed = line.trim();
            let line_lower = line_trimmed.to_lowercase();

            for pattern in &expiry_patterns {
                if line_lower.contains(pattern) {
                    if let Some(colon_pos) = line_trimmed.find(':') {
                        let date_str = line_trimmed[colon_pos + 1..].trim();

                        if let Ok(dt) = DateTime::parse_from_rfc3339(date_str) {
                            return Ok(dt.with_timezone(&Utc));
                        }

                        let formats = vec![
                            "%Y-%m-%d %H:%M:%S",
                            "%Y-%m-%d",
                            "%Y.%m.%d",
                            "%d-%b-%Y",
                            "%d.%m.%Y",
                            "%d/%m/%Y",
                        ];

                        for format in &formats {
                            if let Ok(dt) =
                                NaiveDateTime::parse_from_str(date_str, format)
                            {
                                return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
                            }

                            if let Ok(date) =
                                chrono::NaiveDate::parse_from_str(date_str, format)
                            {
                                let dt = date.and_hms_opt(23, 59, 59).unwrap();
                                return Ok(DateTime::from_naive_utc_and_offset(dt, Utc));
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow!("Could not parse expiry date from WHOIS"))
    }

    async fn notify_ssl_expiration(&mut self, entry: serde_json::Value) {
        for notifier in &mut self.notifiers {
            notifier.ssl_expiration(&entry).await;
        }
    }

    async fn notify_expiration(&mut self, entry: serde_json::Value) {
        for notifier in &mut self.notifiers {
            notifier.expiration(&entry).await;
        }
    }

    async fn commit(&self) -> Result<()> {
        for notifier in &self.notifiers {
            if let Err(e) = notifier.commit().await {
                tracing::error!(dcl = self.dcl, e = %e, "Commit failed");
            }
        }
        Ok(())
    }

    async fn notify_exception(&mut self, msg: &str) {
        for notifier in &mut self.notifiers {
            notifier.exception(msg).await;
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut hostnames: HashSet<String> = HashSet::new();
        let mut source_errors = Vec::new();

        for source in &self.sources {
            match source.get_domains().await {
                Ok(domains) => {
                    hostnames.extend(domains);
                }
                Err(e) => {
                    let source_name = source.get_source_name();
                    tracing::error!(
                        dcl = self.dcl,
                        e = %e,
                        source=source_name,
                        "Ошибка загрузки из источника"
                    );
                    source_errors.push(format!(
                        "Ошибка загрузки из источника: {}.\n{}",
                        source_name, e
                    ));
                }
            }
        }

        for error_msg in source_errors {
            for notifier in &mut self.notifiers {
                notifier.exception(&error_msg).await;
            }
        }

        if hostnames.is_empty() {
            tracing::warn!(dcl = self.dcl, "Не удалось загрузить список доменов");
            return Ok(());
        }

        tracing::info!(dcl = self.dcl, count = hostnames.len(), "Загружены домены");

        let mut expiring_domains: HashMap<String, serde_json::Value> = HashMap::new();
        let mut domain_failed: HashSet<String> = HashSet::new();

        let root_hostnames: HashSet<String> =
            hostnames.iter().filter_map(|h| self.to_root_domain(h)).collect();

        let alarm_days = self.alarm_days;
        let domain_tasks: Vec<_> = root_hostnames
            .into_iter()
            .map(|root| {
                tokio::spawn(async move {
                    let result = Self::check_domain_expiration(&root).await;
                    (root, result)
                })
            })
            .collect();

        let domain_results = join_all(domain_tasks).await;

        for task_result in domain_results {
            if let Ok((root, check_result)) = task_result {
                match check_result {
                    Ok(expiration_date) => {
                        let now = Utc::now();
                        let delta = expiration_date.signed_duration_since(now);
                        let days = delta.num_days();

                        if days < alarm_days || days < 3 {
                            expiring_domains.insert(
                                root.clone(),
                                json!({
                                    "hostname": root,
                                    "expiration_date": expiration_date.to_rfc3339(),
                                    "days": days
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            dcl = self.dcl,
                            domain = root,
                            error = %e,
                            "Ошибка проверки домена"
                        );
                        domain_failed.insert(format!("- {}", root));
                    }
                }
            }
        }

        let mut expiring_ssl: HashMap<String, serde_json::Value> = HashMap::new();
        let mut ssl_failed: HashSet<String> = HashSet::new();
        let ssl_hostnames: HashSet<String> =
            hostnames.iter().filter_map(|h| self.filter_domain(h)).collect();

        let ssl_alarm_days = self.ssl_alarm_days;
        let ssl_tasks: Vec<_> = ssl_hostnames
            .into_iter()
            .map(|hostname| {
                tokio::spawn(async move {
                    let result = Self::check_ssl_expiry(&hostname).await;
                    (hostname, result)
                })
            })
            .collect();

        let ssl_results = join_all(ssl_tasks).await;

        for task_result in ssl_results {
            if let Ok((hostname, check_result)) = task_result {
                match check_result {
                    Ok((expiration_date, serial, issuer)) => {
                        let now = Utc::now();
                        let delta = expiration_date.signed_duration_since(now);
                        let days = delta.num_days();

                        if days <= ssl_alarm_days || days <= 1 {
                            let prev = expiring_ssl.get(&serial);
                            let more = prev
                                .and_then(|v| v.get("more"))
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0)
                                + 1;

                            expiring_ssl.insert(
                                serial.clone(),
                                json!({
                                    "info": {
                                        "serial": serial,
                                        "issuer": issuer
                                    },
                                    "days": days,
                                    "hostname": hostname,
                                    "expiration_date": expiration_date.to_rfc3339(),
                                    "more": if more > 1 { more } else { 1 }
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        let err_str = e.to_string();

                        if !Self::EXPECTED_ERRORS
                            .iter()
                            .any(|exp_err| err_str.contains(exp_err))
                        {
                            ssl_failed.insert(format!("- {}", hostname));
                        }

                        let is_expected = Self::EXPECTED_ERRORS
                            .iter()
                            .any(|exp_err| err_str.contains(exp_err));

                        if is_expected {
                            tracing::debug!(
                                dcl = self.dcl,
                                hostname = hostname,
                                error = %e,
                                "Ожидаемая ошибка SSL (пропускаем)"
                            );
                        } else {
                            tracing::warn!(
                                dcl = self.dcl,
                                hostname = hostname,
                                error = %e,
                                "Неожиданная ошибка SSL"
                            );
                            ssl_failed.insert(format!("- {}", hostname));
                        }
                    }
                }
            }
        }

        if !domain_failed.is_empty() {
            let msg = if domain_failed.len() == 1 {
                format!("Ошибка проверки домена: {:?}", domain_failed)
            } else {
                format!(
                    "Ошибка проверки {} доменов\n{}",
                    domain_failed.len(),
                    domain_failed.into_iter().collect::<Vec<_>>().join("\n")
                )
            };
            self.notify_exception(&msg).await;
        }

        if !ssl_failed.is_empty() {
            let msg = if ssl_failed.len() == 1 {
                format!("Ошибка проверки сертификата: {:?}", ssl_failed)
            } else {
                format!(
                    "Ошибка проверки {} SSL-сертификатов\n{}",
                    ssl_failed.len(),
                    ssl_failed.into_iter().collect::<Vec<_>>().join("\n")
                )
            };
            self.notify_exception(&msg).await;
        }

        let mut expiring_list: Vec<_> = expiring_domains.into_values().collect();
        expiring_list
            .sort_by_key(|v| v.get("days").and_then(|d| d.as_i64()).unwrap_or(0));

        for entry in expiring_list {
            self.notify_expiration(entry).await;
        }

        let mut expiring_ssl_list: Vec<_> = expiring_ssl.into_values().collect();
        expiring_ssl_list
            .sort_by_key(|v| v.get("days").and_then(|d| d.as_i64()).unwrap_or(0));

        for entry in expiring_ssl_list {
            self.notify_ssl_expiration(entry).await;
        }

        self.commit().await?;

        tracing::info!(dcl = self.dcl, "Проверка завершена");

        Ok(())
    }
}
