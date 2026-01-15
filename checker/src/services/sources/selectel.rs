use super::DomainSourceTrait;
use async_trait::async_trait;
use base::prelude::{
    anyhow::{anyhow, Result},
    serde_json,
    tokio::{
        net::TcpStream,
        time::{timeout, Duration},
    },
    tracing,
};
use reqwest::Client;

const ALLOWED_TYPES: &[&str] = &["A", "CNAME"];

pub struct SelectelSourceService {
    account_id: String,
    password: String,
    project_name: String,
    user: String,
    client: Client,
    dcl: &'static str,
}

impl SelectelSourceService {
    pub fn new(account_id: &str, password: &str, project_name: &str, user: &str) -> Self {
        Self {
            account_id: account_id.to_string(),
            password: password.to_string(),
            project_name: project_name.to_string(),
            user: user.to_string(),
            client: Client::new(),
            dcl: "SelectelSourceService",
        }
    }

    #[allow(dead_code)]
    pub async fn has_ssl(host: &str) -> bool {
        let host = match idna::domain_to_ascii(host) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let addr = format!("{}:443", host);
        match timeout(Duration::from_millis(500), TcpStream::connect(addr)).await {
            Ok(Ok(_stream)) => true,
            _ => false,
        }
    }

    async fn get_auth_token(&self) -> Result<String> {
        let body = serde_json::json!({
            "auth": {
                "identity": {
                    "methods": ["password"],
                    "password": {
                        "user": {
                            "name": self.user,
                            "domain": {
                                "name": self.account_id
                            },
                            "password": self.password
                        }
                    }
                },
                "scope": {
                    "project": {
                        "name": self.project_name,
                        "domain": { "name": self.account_id }
                    }
                }
            }
        });
        let resp = self
            .client
            .post("https://cloud.api.selcloud.ru/identity/v3/auth/tokens")
            .json(&body)
            .send()
            .await?;
        let token = resp
            .headers()
            .get("X-Subject-Token")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        if !resp.status().is_success() || token.is_none() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            tracing::error!(
                dcl = self.dcl,
                status = status.to_string(),
                body = text,
                "Не удалось авторизоваться в Selectel"
            );
            return Err(anyhow!("Не удалось авторизоваться в Selectel"));
        }
        Ok(token.unwrap())
    }

    async fn get_zones(&self, token: &str) -> Result<Vec<String>> {
        let resp = self
            .client
            .get("https://api.selectel.ru/domains/v2/zones")
            .header("X-Auth-Token", token)
            .send()
            .await?;
        if !resp.status().is_success() {
            tracing::error!(
                dcl = self.dcl,
                status = resp.status().to_string(),
                "Не удалось получить список зон"
            );
            return Err(anyhow!("Не удалось получить список зон"));
        }
        let json = resp.json::<serde_json::Value>().await?;
        let mut zones = Vec::new();
        if let Some(results) = json.get("result").and_then(|v| v.as_array()) {
            for zone in results {
                let disabled =
                    zone.get("disabled").and_then(|v| v.as_bool()).unwrap_or(true);
                if !disabled {
                    if let Some(id) = zone.get("id").and_then(|v| v.as_str()) {
                        zones.push(id.to_string());
                    }
                }
            }
        }
        Ok(zones)
    }

    async fn get_domains(&self, token: &str, zones: &[String]) -> Result<Vec<String>> {
        let mut domains = Vec::new();
        for zone_id in zones {
            let url = format!("https://api.selectel.ru/domains/v2/zones/{zone_id}/rrset");
            let resp = self.client.get(&url).header("X-Auth-Token", token).send().await?;
            if !resp.status().is_success() {
                tracing::error!("Не удалось получить домены для зоны {zone_id}");
                continue;
            }

            let json = resp.json::<serde_json::Value>().await?;
            if let Some(results) = json.get("result").and_then(|v| v.as_array()) {
                for rec in results {
                    let name = rec.get("name").and_then(|v| v.as_str());
                    let r_type = rec.get("type").and_then(|v| v.as_str());
                    let disabled = rec
                        .get("records")
                        .and_then(|v| v.get(0))
                        .and_then(|v| v.get("disabled"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    if let (Some(name), Some(r_type)) = (name, r_type) {
                        if ALLOWED_TYPES.contains(&r_type) && !disabled {
                            domains.push(name.trim_end_matches('.').to_string());
                        }
                    }
                }
            }
        }
        Ok(domains)
    }
}

#[async_trait]
impl DomainSourceTrait for SelectelSourceService {
    async fn get_domains(&self) -> Result<Vec<String>> {
        let token = self.get_auth_token().await?;
        let zones = self.get_zones(&token).await?;
        let domains = self.get_domains(&token, &zones).await?;
        Ok(domains)
    }

    fn get_source_name(&self) -> &'static str {
        self.dcl
    }
}
