use super::span_fields_layer::SpanFields;
use chrono::Local;
use colored::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::fmt;
use tracing::{Event, Subscriber};
use tracing_subscriber::{
    fmt::{format::Writer, FmtContext, FormatEvent, FormatFields},
    registry::LookupSpan,
};

pub struct ColorfulFormatter {
    pub use_color: bool,
}

impl ColorfulFormatter {
    pub fn new(use_color: bool) -> Self {
        Self { use_color }
    }
}

impl<S, N> FormatEvent<S, N> for ColorfulFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        let level = event.metadata().level();

        // Собираем поля из event
        let mut fields = IndexMap::new();
        let mut decl = "app".to_string();
        let mut message = String::new();

        let mut visitor = FieldVisitor {
            fields: &mut fields,
            decl: &mut decl,
            message: &mut message,
        };
        event.record(&mut visitor);

        // Собираем поля из span'ов
        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                let extensions = span.extensions();
                if let Some(span_fields) = extensions.get::<SpanFields>() {
                    for (key, value) in &span_fields.fields {
                        if !fields.contains_key(key) && key != "message" {
                            fields.insert(key.clone(), value.clone());
                        }
                    }
                }
            }
        }

        // Формируем строку
        let level_str = match *level {
            tracing::Level::ERROR => "ERROR",
            tracing::Level::WARN => "WARN",
            tracing::Level::INFO => "INFO",
            tracing::Level::DEBUG => "DEBUG",
            tracing::Level::TRACE => "TRACE",
        };

        let mut log_line =
            format!("[{}] [{}] {}: {}", timestamp, decl, level_str, message);

        if !fields.is_empty() {
            let json_str = serde_json::to_string(&fields).unwrap_or_default();
            log_line.push_str(&format!(" -> {}", json_str));
        }

        // Красим
        if self.use_color {
            let colored = match *level {
                tracing::Level::ERROR => log_line.red().bold(),
                tracing::Level::WARN => log_line.yellow(),
                tracing::Level::INFO => log_line.blue(),
                tracing::Level::DEBUG => log_line.white(),
                tracing::Level::TRACE => log_line.bright_black(),
            };
            writeln!(writer, "{}", colored)
        } else {
            writeln!(writer, "{}", log_line)
        }
    }
}

struct FieldVisitor<'a> {
    fields: &'a mut IndexMap<String, Value>,
    decl: &'a mut String,
    message: &'a mut String,
}

impl<'a> tracing::field::Visit for FieldVisitor<'a> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        match field.name() {
            "message" => *self.message = format!("{:?}", value),
            "decl" => {
                *self.decl = format!("{:?}", value).trim_matches('"').to_string()
            }
            _ => {
                self.fields
                    .insert(field.name().to_string(), json!(format!("{:?}", value)));
            }
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => *self.message = value.to_string(),
            "dcl" => *self.decl = value.to_string(),
            _ => {
                self.fields.insert(field.name().to_string(), json!(value));
            }
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() != "message" && field.name() != "dcl" {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() != "message" && field.name() != "dcl" {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if field.name() != "message" && field.name() != "dcl" {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if field.name() != "message" && field.name() != "dcl" {
            self.fields.insert(field.name().to_string(), json!(value));
        }
    }
}
