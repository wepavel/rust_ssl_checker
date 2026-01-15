use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod formatter;
mod logstash;
mod span_fields_layer;

use crate::config::LogConfig;
use colored::control;
use formatter::ColorfulFormatter;
use logstash::LogstashLayer;
use span_fields_layer::SpanFieldsLayer;

/// Инициализация глобального логгера
pub async fn init_logging(config: &LogConfig) -> anyhow::Result<()> {
    let span_fields = SpanFieldsLayer::default();
    control::set_override(config.use_color);

    let console = tracing_subscriber::fmt::layer()
        .event_format(ColorfulFormatter::new(config.use_color))
        .with_writer(std::io::stdout);

    let env_filter = EnvFilter::new(&config.log_level);

    let subscriber =
        tracing_subscriber::registry().with(env_filter).with(span_fields).with(console);

    // Добавляем Logstash если настроен
    if let (Some(host), Some(port), Some(app_name)) =
        (&config.logstash_host, config.logstash_port, &config.app_name)
    {
        let logstash = LogstashLayer::new(&host, port, app_name).await?;
        subscriber.with(logstash).init();
    } else {
        subscriber.init();
    }

    Ok(())
}
