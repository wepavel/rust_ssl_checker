use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LogConfig {
    pub log_level: String,
    pub use_color: bool,
    pub logstash_host: Option<String>,
    pub logstash_port: Option<u16>,
    pub app_name: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            log_level: "info".to_string(),
            use_color: false,
            logstash_host: None,
            logstash_port: None,
            app_name: None,
        }
    }
}
