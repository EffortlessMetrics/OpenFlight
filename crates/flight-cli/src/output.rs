// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Output formatting utilities for CLI

use clap::ValueEnum;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON output
    Json,
}

impl OutputFormat {
    /// Format a success response
    pub fn success(&self, data: Value) -> String {
        match self {
            OutputFormat::Json => json!({
                "success": true,
                "data": data
            })
            .to_string(),
            OutputFormat::Human => format_human_output(&data),
        }
    }

    /// Format an error response
    pub fn error(&self, message: &str, error_code: &str) -> String {
        match self {
            OutputFormat::Json => json!({
                "success": false,
                "error": message,
                "error_code": error_code
            })
            .to_string(),
            OutputFormat::Human => {
                format!("Error: {}", message)
            }
        }
    }

    /// Format a list of items
    pub fn list(&self, items: Vec<Value>, total_count: Option<i32>) -> String {
        match self {
            OutputFormat::Json => {
                let mut result = json!({
                    "success": true,
                    "data": items
                });

                if let Some(count) = total_count {
                    result["total_count"] = json!(count);
                }

                result.to_string()
            }
            OutputFormat::Human => {
                let mut output = String::new();

                if let Some(count) = total_count {
                    output.push_str(&format!("Total: {} items\n\n", count));
                }

                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        output.push('\n');
                    }
                    output.push_str(&format_human_output(item));
                }

                output
            }
        }
    }
}

fn format_human_output(data: &Value) -> String {
    match data {
        Value::Object(map) => {
            let mut output = String::new();

            for (key, value) in map {
                match value {
                    Value::String(s) => output.push_str(&format!("{}: {}\n", key, s)),
                    Value::Number(n) => output.push_str(&format!("{}: {}\n", key, n)),
                    Value::Bool(b) => output.push_str(&format!("{}: {}\n", key, b)),
                    Value::Array(arr) => {
                        output.push_str(&format!("{}:\n", key));
                        for item in arr {
                            output.push_str(&format!("  - {}\n", format_human_output(item).trim()));
                        }
                    }
                    Value::Object(_) => {
                        output.push_str(&format!("{}:\n", key));
                        let nested = format_human_output(value);
                        for line in nested.lines() {
                            output.push_str(&format!("  {}\n", line));
                        }
                    }
                    Value::Null => output.push_str(&format!("{}: null\n", key)),
                }
            }

            output
        }
        Value::Array(arr) => {
            let mut output = String::new();
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    output.push('\n');
                }
                output.push_str(&format!("Item {}:\n", i + 1));
                let item_output = format_human_output(item);
                for line in item_output.lines() {
                    output.push_str(&format!("  {}\n", line));
                }
            }
            output
        }
        _ => data.to_string(),
    }
}

/// Helper to convert protobuf messages to JSON values
pub fn proto_to_json<T: serde::Serialize>(proto: &T) -> anyhow::Result<Value> {
    let json_str = serde_json::to_string(proto)?;
    let value: Value = serde_json::from_str(&json_str)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_json_is_valid_json_with_success_true() {
        let result = OutputFormat::Json.success(json!({"key": "value"}));
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["key"], "value");
    }

    #[test]
    fn success_human_is_non_empty() {
        let result = OutputFormat::Human.success(json!({"key": "value"}));
        assert!(!result.is_empty());
        assert!(result.contains("key"));
    }

    #[test]
    fn error_json_is_valid_json_with_success_false() {
        let result = OutputFormat::Json.error("something went wrong", "TEST_ERROR");
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"], "something went wrong");
        assert_eq!(parsed["error_code"], "TEST_ERROR");
    }

    #[test]
    fn error_human_starts_with_error_prefix() {
        let result = OutputFormat::Human.error("something went wrong", "TEST_ERROR");
        assert!(result.starts_with("Error:"));
        assert!(result.contains("something went wrong"));
    }

    #[test]
    fn list_json_empty_is_valid_json() {
        let result = OutputFormat::Json.list(vec![], None);
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"], json!([]));
    }

    #[test]
    fn list_json_includes_total_count_when_provided() {
        let result = OutputFormat::Json.list(vec![json!({"id": "1"})], Some(5));
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["total_count"], 5);
    }

    #[test]
    fn list_json_omits_total_count_when_none() {
        let result = OutputFormat::Json.list(vec![], None);
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.get("total_count").is_none());
    }

    #[test]
    fn list_human_empty_returns_empty_string() {
        let result = OutputFormat::Human.list(vec![], None);
        assert!(result.is_empty());
    }

    #[test]
    fn list_human_with_count_shows_total() {
        let result = OutputFormat::Human.list(vec![], Some(3));
        assert!(result.contains("3"));
    }

    #[test]
    fn success_json_wraps_array_data() {
        let result = OutputFormat::Json.success(json!(["a", "b"]));
        let parsed: Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["data"].is_array());
    }
}
