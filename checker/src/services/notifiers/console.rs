use super::BaseNotifierTrait;
use async_trait::async_trait;
use base::prelude::{
    anyhow::{self, Result},
    serde_json::Value,
    tracing,
};

pub struct ConsoleNotifierService {
    pub ssl_entries: Vec<Value>,
    pub domain_entries: Vec<Value>,
    pub errors: Vec<String>,
    dcl: &'static str,
}

impl ConsoleNotifierService {
    pub fn new() -> Self {
        Self {
            ssl_entries: Vec::new(),
            domain_entries: Vec::new(),
            errors: Vec::new(),
            dcl: "ConsoleNotifierService",
        }
    }

    fn format_ssl_entries(&self) -> Vec<String> {
        let mut messages = Vec::new();

        for entry in &self.ssl_entries {
            let result: Result<String> = (|| {
                // Извлекаем данные из JSON
                let serial = entry
                    .get("info")
                    .and_then(|v| v.get("serial"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing serial"))?;

                let issuer = entry
                    .get("info")
                    .and_then(|v| v.get("issuer"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing issuer"))?;

                let hostname = entry
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing hostname"))?;

                let days = entry
                    .get("days")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| anyhow::anyhow!("Missing days"))?
                    as i32;

                let day_word = self.format_days(days);

                let more_info = entry
                    .get("more")
                    .and_then(|v| v.as_str())
                    .map(|m| format!(" (+{})", m))
                    .unwrap_or_default();

                let msg = if days >= 0 {
                    format!(
                        "Сертификат {} ({}) истекает через: {} {} для {}{}",
                        serial, issuer, days, day_word, hostname, more_info
                    )
                } else {
                    format!(
                        "Сертификат {} ({}) истёк: {} {} назад для {}{}",
                        serial,
                        issuer,
                        days.abs(),
                        day_word,
                        hostname,
                        more_info
                    )
                };

                Ok(msg)
            })();

            match result {
                Ok(msg) => messages.push(msg),
                Err(e) => eprintln!("ERROR formatting SSL entry: {}", e),
            }
        }

        messages
    }

    fn format_domain_entries(&self) -> Vec<String> {
        let mut messages = Vec::new();

        for entry in &self.domain_entries {
            let result: Result<String> = (|| {
                let hostname = entry
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing hostname"))?;

                let days = entry
                    .get("days")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| anyhow::anyhow!("Missing days"))?
                    as i32;

                let day_word = self.format_days(days);

                let msg = if days >= 0 {
                    format!("- Домен {} истекает через {} {}", hostname, days, day_word)
                } else {
                    format!("Домен истёк: {} {} назад", days.abs(), day_word)
                };

                Ok(msg)
            })();

            match result {
                Ok(msg) => messages.push(msg),
                Err(e) => eprintln!("ERROR formatting domain entry: {}", e),
            }
        }

        messages
    }

    fn format_errors(&self) -> Vec<String> {
        self.errors.iter().map(|err| err.to_string()).collect()
    }
}

#[async_trait]
impl BaseNotifierTrait for ConsoleNotifierService {
    async fn ssl_expiration(&mut self, entry: &Value) {
        self.ssl_entries.push(entry.clone());
    }
    async fn exception(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }
    async fn expiration(&mut self, entry: &Value) {
        self.domain_entries.push(entry.clone());
    }

    async fn commit(&self) -> Result<()> {
        let ssl_messages = self.format_ssl_entries();
        let domain_messages = self.format_domain_entries();
        let error_messages = self.format_errors();

        if ssl_messages.is_empty()
            && domain_messages.is_empty()
            && error_messages.is_empty()
        {
            tracing::warn!(dcl = self.dcl, "Отсутствуют сообщения для отправки");
            return Ok(());
        }

        if !ssl_messages.is_empty() {
            tracing::warn!(
                dcl = self.dcl,
                "Срок действия SSL‑сертификатов истекает:\n{}",
                ssl_messages.join("\n")
            );
        }

        if !domain_messages.is_empty() {
            tracing::warn!(
                dcl = self.dcl,
                "Срок действия доменов истекает:\n{}",
                domain_messages.join("\n")
            );
        }

        if !error_messages.is_empty() {
            tracing::error!(
                dcl = self.dcl,
                "Произошли ошибки:\n{}",
                error_messages.join("\n")
            );
        }

        Ok(())
    }
}
