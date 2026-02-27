// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Structured log entry builder and JSON log formatter.
//!
//! Provides [`LogEntry`] with a [`LogEntryBuilder`] for constructing typed,
//! field-rich log records, and [`JsonLogFormatter`] for serialising them as
//! newline-delimited JSON.

use std::collections::BTreeMap;
use std::fmt;
use std::time::SystemTime;

/// A structured log entry with typed fields.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: SystemTime,
    pub level: LogLevel,
    pub component: String,
    pub message: String,
    pub fields: BTreeMap<String, LogValue>,
    pub span_id: Option<String>,
    pub trace_id: Option<String>,
}

/// Severity level for a [`LogEntry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trace => f.write_str("TRACE"),
            Self::Debug => f.write_str("DEBUG"),
            Self::Info => f.write_str("INFO"),
            Self::Warn => f.write_str("WARN"),
            Self::Error => f.write_str("ERROR"),
        }
    }
}

/// A dynamically-typed value attached to a [`LogEntry`] field.
#[derive(Debug, Clone)]
pub enum LogValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl fmt::Display for LogValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String(s) => f.write_str(s),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// Builder for constructing [`LogEntry`] instances.
pub struct LogEntryBuilder {
    level: LogLevel,
    component: String,
    message: String,
    fields: BTreeMap<String, LogValue>,
    span_id: Option<String>,
    trace_id: Option<String>,
}

impl LogEntryBuilder {
    /// Create a new builder with the required fields.
    pub fn new(level: LogLevel, component: &str, message: &str) -> Self {
        Self {
            level,
            component: component.to_owned(),
            message: message.to_owned(),
            fields: BTreeMap::new(),
            span_id: None,
            trace_id: None,
        }
    }

    /// Attach a typed field.
    pub fn field(mut self, key: &str, value: LogValue) -> Self {
        self.fields.insert(key.to_owned(), value);
        self
    }

    /// Set the span identifier.
    pub fn span_id(mut self, id: &str) -> Self {
        self.span_id = Some(id.to_owned());
        self
    }

    /// Set the trace identifier.
    pub fn trace_id(mut self, id: &str) -> Self {
        self.trace_id = Some(id.to_owned());
        self
    }

    /// Consume the builder and produce a [`LogEntry`].
    pub fn build(self) -> LogEntry {
        LogEntry {
            timestamp: SystemTime::now(),
            level: self.level,
            component: self.component,
            message: self.message,
            fields: self.fields,
            span_id: self.span_id,
            trace_id: self.trace_id,
        }
    }
}

/// Formats [`LogEntry`] values as JSON lines.
pub struct JsonLogFormatter;

impl JsonLogFormatter {
    /// Serialise a single entry as a one-line JSON string.
    pub fn format(entry: &LogEntry) -> String {
        let mut map = serde_json::Map::new();

        let epoch = entry
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        map.insert(
            "timestamp".into(),
            serde_json::Value::String(format!("{}.{:03}", epoch.as_secs(), epoch.subsec_millis())),
        );
        map.insert(
            "level".into(),
            serde_json::Value::String(entry.level.to_string()),
        );
        map.insert(
            "component".into(),
            serde_json::Value::String(entry.component.clone()),
        );
        map.insert(
            "message".into(),
            serde_json::Value::String(entry.message.clone()),
        );

        if !entry.fields.is_empty() {
            let fields_obj: serde_json::Map<String, serde_json::Value> = entry
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), log_value_to_json(v)))
                .collect();
            map.insert("fields".into(), serde_json::Value::Object(fields_obj));
        }

        if let Some(ref id) = entry.span_id {
            map.insert("span_id".into(), serde_json::Value::String(id.clone()));
        }
        if let Some(ref id) = entry.trace_id {
            map.insert("trace_id".into(), serde_json::Value::String(id.clone()));
        }

        serde_json::Value::Object(map).to_string()
    }

    /// Serialise a batch of entries as newline-delimited JSON.
    pub fn format_batch(entries: &[LogEntry]) -> String {
        entries
            .iter()
            .map(Self::format)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn log_value_to_json(value: &LogValue) -> serde_json::Value {
    match value {
        LogValue::String(s) => serde_json::Value::String(s.clone()),
        LogValue::Int(i) => serde_json::json!(*i),
        LogValue::Float(f) => serde_json::json!(*f),
        LogValue::Bool(b) => serde_json::Value::Bool(*b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_creates_valid_entry() {
        let entry = LogEntryBuilder::new(LogLevel::Info, "axis", "tick processed").build();

        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.component, "axis");
        assert_eq!(entry.message, "tick processed");
        assert!(entry.fields.is_empty());
        assert!(entry.span_id.is_none());
        assert!(entry.trace_id.is_none());
    }

    #[test]
    fn fields_included_in_entry() {
        let entry = LogEntryBuilder::new(LogLevel::Debug, "hid", "write complete")
            .field("device_id", LogValue::Int(42))
            .field("success", LogValue::Bool(true))
            .build();

        assert_eq!(entry.fields.len(), 2);
        assert!(matches!(
            entry.fields.get("device_id"),
            Some(LogValue::Int(42))
        ));
        assert!(matches!(
            entry.fields.get("success"),
            Some(LogValue::Bool(true))
        ));
    }

    #[test]
    fn json_format_includes_all_fields() {
        let entry = LogEntryBuilder::new(LogLevel::Warn, "ffb", "force clamp")
            .field("force_n", LogValue::Float(9.5))
            .span_id("span-1")
            .trace_id("trace-1")
            .build();

        let json = JsonLogFormatter::format(&entry);

        assert!(json.contains("\"level\":\"WARN\""));
        assert!(json.contains("\"component\":\"ffb\""));
        assert!(json.contains("\"message\":\"force clamp\""));
        assert!(json.contains("\"span_id\":\"span-1\""));
        assert!(json.contains("\"trace_id\":\"trace-1\""));
        assert!(json.contains("\"force_n\":9.5"));
    }

    #[test]
    fn json_format_is_valid_json() {
        let entry = LogEntryBuilder::new(LogLevel::Error, "sched", "deadline miss")
            .field("tick", LogValue::Int(100))
            .build();

        let json = JsonLogFormatter::format(&entry);
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("output must be valid JSON");

        assert_eq!(parsed["level"], "ERROR");
        assert_eq!(parsed["component"], "sched");
        assert_eq!(parsed["fields"]["tick"], 100);
    }

    #[test]
    fn batch_format_produces_newline_delimited() {
        let entries: Vec<LogEntry> = (0..3)
            .map(|i| LogEntryBuilder::new(LogLevel::Info, "test", &format!("msg {i}")).build())
            .collect();

        let batch = JsonLogFormatter::format_batch(&entries);
        let lines: Vec<&str> = batch.lines().collect();

        assert_eq!(lines.len(), 3);
        for line in &lines {
            serde_json::from_str::<serde_json::Value>(line).expect("each line must be valid JSON");
        }
    }

    #[test]
    fn log_levels_ordering_correct() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn optional_span_and_trace_ids() {
        let without = LogEntryBuilder::new(LogLevel::Info, "a", "b").build();
        let json_without = JsonLogFormatter::format(&without);
        assert!(!json_without.contains("span_id"));
        assert!(!json_without.contains("trace_id"));

        let with = LogEntryBuilder::new(LogLevel::Info, "a", "b")
            .span_id("s1")
            .trace_id("t1")
            .build();
        let json_with = JsonLogFormatter::format(&with);
        assert!(json_with.contains("\"span_id\":\"s1\""));
        assert!(json_with.contains("\"trace_id\":\"t1\""));
    }

    #[test]
    fn log_value_display_formats_correctly() {
        assert_eq!(LogValue::String("hello".into()).to_string(), "hello");
        assert_eq!(LogValue::Int(42).to_string(), "42");
        assert_eq!(LogValue::Float(3.14).to_string(), "3.14");
        assert_eq!(LogValue::Bool(true).to_string(), "true");
        assert_eq!(LogValue::Bool(false).to_string(), "false");
    }

    #[test]
    fn empty_fields_handled() {
        let entry = LogEntryBuilder::new(LogLevel::Trace, "core", "startup").build();
        let json = JsonLogFormatter::format(&entry);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // No "fields" key when empty
        assert!(parsed.get("fields").is_none());
    }

    #[test]
    fn special_characters_escaped_in_json() {
        let entry = LogEntryBuilder::new(LogLevel::Info, "test", "line1\nline2\ttab \"quoted\"")
            .field("path", LogValue::String("C:\\Users\\test".into()))
            .build();

        let json = JsonLogFormatter::format(&entry);
        // Must be parseable (serde_json escapes automatically)
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("special chars must be properly escaped");

        assert_eq!(parsed["message"], "line1\nline2\ttab \"quoted\"");
        assert_eq!(parsed["fields"]["path"], "C:\\Users\\test");
    }

    #[test]
    fn batch_format_empty_slice() {
        let batch = JsonLogFormatter::format_batch(&[]);
        assert!(batch.is_empty());
    }
}
