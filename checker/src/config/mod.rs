use base::config::LogConfig;
use base::prelude::{
    config::{Config, Environment, File},
    once_cell::sync::Lazy,
    anyhow::Result
};
use std::collections::HashMap;
use serde::Deserialize;

pub static CONFIG: Lazy<ServiceConfig> =
    Lazy::new(|| ServiceConfig::load().expect("Failed to load config"));

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SourceConfig {
    FileConfig {
        filename: String,
    },
    SelectelConfig {
        account_id: String,
        password: String,
        project_name: String,
        user: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum NotifierConfig {
    Telegram {
        bot_token: String,
        chat_id: String,
        #[serde(default = "NotifierConfig::default_retries")]
        retries: u32,
    },
    Console,
}

impl NotifierConfig {
    fn default_retries() -> u32 { 5 }
}


#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    #[serde(default)]
    pub log_config: LogConfig,
    pub check_interval_hours: u64,
    pub notifiers: HashMap<String, NotifierConfig>,
    pub sources: HashMap<String, SourceConfig>,
    pub alarm_days: i64,
    pub ssl_alarm_days: i64,
}

impl ServiceConfig {
    pub fn load() -> Result<Self> {
        let env_path = std::env::var("CONFIG_PATH").unwrap_or("config.yml".to_string());

        let config: Self = Config::builder()
            .add_source(File::with_name(&env_path).required(false))
            .add_source(Environment::with_prefix("APP").separator("."))
            .build()?
            .try_deserialize()?;

        Ok(config)
    }
}