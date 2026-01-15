use serde_json::{json, Map, Value};
use std::fmt;
use tracing::field::{Field, Visit};
use tracing::{span, Subscriber};
use tracing_subscriber::{registry::LookupSpan, Layer};

#[derive(Debug, Clone, Default)]
pub struct SpanFields {
    pub fields: Map<String, Value>,
}

impl Visit for SpanFields {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields.insert(field.name().to_string(), json!(value));
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(field.name().to_string(), json!(value));
    }
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields.insert(field.name().to_string(), json!(value));
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields.insert(field.name().to_string(), json!(value));
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.insert(field.name().to_string(), json!(value));
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        let debug_str = format!("{:?}", value);
        self.fields.insert(field.name().to_string(), json!(debug_str.trim_matches('"')));
    }
}

#[derive(Default)]
pub struct SpanFieldsLayer;

impl<S> Layer<S> for SpanFieldsLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &span::Attributes<'_>,
        id: &span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("Span not found");
        let mut fields = SpanFields::default();
        attrs.record(&mut fields);
        span.extensions_mut().insert(fields);
    }
}
