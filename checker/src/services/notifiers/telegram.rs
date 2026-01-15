use super::BaseNotifierTrait;
use async_trait::async_trait;
use base::prelude::{
    anyhow::{self, Result},
    serde_json::{json, Value},
    tokio,
};
use reqwest::Client;
use std::time::Duration;

pub struct TelegramNotifierService {
    ssl_entries: Vec<Value>,
    domain_entries: Vec<Value>,
    errors: Vec<String>,
    bot_token: String,
    chat_id: String,
    retries: u32,
    retry_interval: Duration,
    api_url: String,
    client: Client,
}

impl TelegramNotifierService {
    const MAX_MESSAGE_LENGTH: usize = 4096;

    pub fn new(
        bot_token: &str,
        chat_id: &str,
        retries: Option<u32>,
        retry_interval_secs: Option<u64>,
    ) -> Self {
        let retries = retries.unwrap_or(5);
        let retry_interval_secs =
            Duration::from_secs(retry_interval_secs.unwrap_or(1));

        let api_url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);

        let client = Client::builder()
            .timeout(Duration::from_secs(3))
            .connect_timeout(Duration::from_secs(1))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            ssl_entries: Vec::new(),
            domain_entries: Vec::new(),
            errors: Vec::new(),
            bot_token: bot_token.to_string(),
            chat_id: chat_id.to_string(),
            retries,
            retry_interval: retry_interval_secs,
            api_url,
            client,
        }
    }

    /// Разбивает сообщения на чанки по лимиту Telegram
    fn chunk_messages(&self, header: &str, messages: &[String]) -> Vec<Vec<String>> {
        let separator_length = 2;
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_length = header.len() + separator_length;

        for msg in messages {
            let msg_length = msg.len() + separator_length;

            if current_length + msg_length > Self::MAX_MESSAGE_LENGTH {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk);
                    current_chunk = Vec::new();
                }
                current_chunk.push(msg.clone());
                current_length =
                    header.len() + separator_length + msg.len() + separator_length;
            } else {
                current_chunk.push(msg.clone());
                current_length += msg_length;
            }
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }

    /// Отправляет список сообщений с заголовком
    async fn send_messages(&self, header: &str, messages: Vec<String>) -> Result<()> {
        let chunks = self.chunk_messages(header, &messages);

        for (i, chunk) in chunks.iter().enumerate() {
            let prefix = if chunks.len() > 1 {
                format!("[{}/{}] ", i + 1, chunks.len())
            } else {
                String::new()
            };

            let text = format!("{}{}\n\n{}", prefix, header, chunk.join("\n\n"));
            self.send_message(&text).await?;
        }

        Ok(())
    }

    /// Отправляет одно сообщение в Telegram с retry логикой
    async fn send_message(&self, text: &str) -> Result<()> {
        for attempt in 0..=self.retries {
            match self
                .client
                .post(&self.api_url)
                .json(&json!({
                    "chat_id": &self.chat_id,
                    "text": text,
                    "parse_mode": "HTML",
                    "disable_web_page_preview": true
                }))
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(());
                    } else {
                        eprintln!(
                            "ERROR: Telegram API returned status: {}",
                            response.status()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: Ошибка отправки сообщения в Telegram: {}", e);
                }
            }

            if attempt == self.retries {
                eprintln!("ERROR: Превышено количество попыток отправки");
                return Err(anyhow::anyhow!(
                    "Failed to send message after {} retries",
                    self.retries
                ));
            }

            tokio::time::sleep(self.retry_interval).await;
        }

        Ok(())
    }

    /// Форматирует информацию о SSL сертификатах
    fn format_ssl_entries(&self) -> Vec<String> {
        let mut messages = Vec::new();

        for entry in &self.ssl_entries {
            let result: Result<String> = (|| {
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
                let issuer = html_escape::encode_text(issuer);

                let hostname = entry
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing hostname"))?;
                let hostname_escaped = html_escape::encode_text(hostname);

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

                let exp_words = if days >= 0 {
                    format!("Истекает через: <b>{} {}</b>", days, day_word)
                } else {
                    format!("Истёк: <b>{} {} назад</b>", days.abs(), day_word)
                };

                let icon = if days > 2 { "🟡" } else { "🔴" };

                let url = format!("https://{}", hostname);
                let text = format!(
                    "{} <b>Сертификат {}</b>\n\
                    ├ Издатель: <code>{}</code>\n\
                    ├ Хост: <a href=\"{}\">{}</a>{}\n\
                    └ {}",
                    icon, serial, issuer, url, hostname_escaped, more_info, exp_words
                );

                Ok(text)
            })();

            match result {
                Ok(msg) => messages.push(msg),
                Err(e) => eprintln!("ERROR formatting SSL entry: {}", e),
            }
        }

        messages
    }

    /// Форматирует информацию о доменах
    fn format_domain_entries(&self) -> Vec<String> {
        let mut messages = Vec::new();

        for entry in &self.domain_entries {
            let result: Result<String> = (|| {
                let hostname = entry
                    .get("hostname")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing hostname"))?;
                let hostname_escaped = html_escape::encode_text(hostname);

                let days = entry
                    .get("days")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| anyhow::anyhow!("Missing days"))?
                    as i32;

                let day_word = self.format_days(days);

                let exp_words = if days >= 0 {
                    format!("Истекает через: <b>{} {}</b>", days, day_word)
                } else {
                    format!("Истёк: <b>{} {} назад</b>", days.abs(), day_word)
                };

                let icon = if days > 2 { "🟡" } else { "🔴" };

                let url = format!("https://{}", hostname);
                let text = format!(
                    "{} <b>Домен</b>: <a href=\"{}\">{}</a>\n└ {}",
                    icon, url, hostname_escaped, exp_words
                );

                Ok(text)
            })();

            match result {
                Ok(msg) => messages.push(msg),
                Err(e) => eprintln!("ERROR formatting domain entry: {}", e),
            }
        }

        messages
    }

    /// Форматирует список ошибок
    fn format_errors(&self) -> Vec<String> {
        self.errors
            .iter()
            .map(|err| {
                let escaped = html_escape::encode_text(err);
                format!("🔴 <code>{}</code>", escaped)
            })
            .collect()
    }
}

#[async_trait]
impl BaseNotifierTrait for TelegramNotifierService {
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

        if !ssl_messages.is_empty() {
            self.send_messages(
                "⚠️ <b>Срок действия SSL‑сертификатов истекает:</b>",
                ssl_messages,
            )
            .await?;
        }

        if !domain_messages.is_empty() {
            self.send_messages(
                "⚠️ <b>Срок действия доменов истекает:</b>",
                domain_messages,
            )
            .await?;
        }

        if !error_messages.is_empty() {
            self.send_messages("🔴 <b>Произошли ошибки:</b>", error_messages).await?;
        }

        Ok(())
    }
}
