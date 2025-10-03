// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Output formatting utilities for CLI

use clap::ValueEnum;
use serde_json::{json, Value};

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
            OutputFormat::Json => {
                json!({
                    "success": true,
                    "data": data
                }).to_string()
            }
            OutputFormat::Human => {
                format_human_output(&data)
            }
        }
    }
    
    /// Format an error response
    pub fn error(&self, message: &str, error_code: &str) -> String {
        match self {
            OutputFormat::Json => {
                json!({
                    "success": false,
                    "error": message,
                    "error_code": error_code
                }).to_string()
            }
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