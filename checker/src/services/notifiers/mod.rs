use async_trait::async_trait;

mod console;
mod telegram;
pub use console::ConsoleNotifierService;
pub use telegram::TelegramNotifierService;

use base::prelude::{anyhow::Result, serde_json::Value};

#[async_trait]
pub trait BaseNotifierTrait: Send + Sync {
    /// Добавление SSL-записи
    async fn ssl_expiration(&mut self, entry: &Value);

    /// Добавление ошибки
    async fn exception(&mut self, msg: &str);

    /// Добавление обычной записи (домены)
    async fn expiration(&mut self, entry: &Value);

    /// Обязательный метод — аналог commit()
    async fn commit(&self) -> Result<()>;

    /// Вспомогательный метод (не async)
    fn format_days(&self, n: i32) -> &'static str {
        let n = n.abs();
        if (11..=14).contains(&(n % 100)) {
            return "дней";
        }
        match n % 10 {
            1 => "день",
            2 | 3 | 4 => "дня",
            _ => "дней",
        }
    }
}
