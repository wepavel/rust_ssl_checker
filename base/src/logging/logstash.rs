use chrono::Utc;
use serde_json::{json, Map, Value};
use std::net::SocketAddr;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

pub struct LogstashLayer {
    addr: SocketAddr,
    app_name: String,
}

impl LogstashLayer {
    pub async fn new(host: &str, port: u16, app_name: &str) -> anyhow::Result<Self> {
        let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
        let _ = TcpStream::connect(addr).await?;
        let app_name = app_name.to_string();
        Ok(Self { addr, app_name })
    }
}

impl<S> Layer<S> for LogstashLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();

        let mut fields = Map::new();
        let mut visitor = JsonVisitor(&mut fields);
        event.record(&mut visitor);

        let log_entry = json!({
            "@timestamp": Utc::now().to_rfc3339(),
            "app": self.app_name,
            "level": metadata.level().to_string(),
            "target": metadata.target(),
            "message": fields.remove("message").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default(),
            "fields": fields,
        });

        let addr = self.addr;
        tokio::spawn(async move {
            if let Ok(mut conn) = TcpStream::connect(addr).await {
                let mut msg = serde_json::to_vec(&log_entry).unwrap_or_default();
                msg.push(b'\n');
                let _ = conn.write_all(&msg).await;
            }
        });
    }
}

struct JsonVisitor<'a>(&'a mut Map<String, Value>);

impl<'a> tracing::field::Visit for JsonVisitor<'a> {
    fn record_debug(
        &mut self,
        field: &tracing::field::Field,
        value: &dyn std::fmt::Debug,
    ) {
        self.0.insert(field.name().to_string(), json!(format!("{:?}", value)));
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.0.insert(field.name().to_string(), json!(value));
    }
    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.0.insert(field.name().to_string(), json!(value));
    }
}
