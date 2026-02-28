// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Service Configuration Validator
//!
//! Validates service configuration before startup using an extensible
//! set of checks. Built-in checks cover port ranges, required fields,
//! numeric ranges, and string-format (regex) validation.

use serde_json::Value;

/// Outcome of a single check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckResult {
    Pass,
    Error(String),
    Warning(String),
}

/// A validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub check: String,
    pub message: String,
}

/// A validation warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWarning {
    pub check: String,
    pub message: String,
}

/// Aggregate validation result.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ConfigError>,
    pub warnings: Vec<ConfigWarning>,
}

/// Trait for individual configuration checks.
pub trait ConfigCheck: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self, config: &Value) -> CheckResult;
}

/// Orchestrates multiple [`ConfigCheck`] implementations.
pub struct ConfigValidator {
    checks: Vec<Box<dyn ConfigCheck>>,
}

impl ConfigValidator {
    #[must_use]
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    pub fn add_check(&mut self, check: Box<dyn ConfigCheck>) -> &mut Self {
        self.checks.push(check);
        self
    }

    /// Run all registered checks and return the aggregate result.
    #[must_use]
    pub fn validate(&self, config: &Value) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        for check in &self.checks {
            match check.check(config) {
                CheckResult::Pass => {}
                CheckResult::Error(msg) => errors.push(ConfigError {
                    check: check.name().to_string(),
                    message: msg,
                }),
                CheckResult::Warning(msg) => warnings.push(ConfigWarning {
                    check: check.name().to_string(),
                    message: msg,
                }),
            }
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        }
    }
}

impl Default for ConfigValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in checks
// ---------------------------------------------------------------------------

/// Validates that a numeric field at `pointer` is a valid TCP/UDP port.
pub struct PortRangeCheck {
    pointer: String,
}

impl PortRangeCheck {
    #[must_use]
    pub fn new(json_pointer: &str) -> Self {
        Self {
            pointer: json_pointer.to_string(),
        }
    }
}

impl ConfigCheck for PortRangeCheck {
    fn name(&self) -> &str {
        "port_range"
    }

    fn check(&self, config: &Value) -> CheckResult {
        match config.pointer(&self.pointer) {
            Some(Value::Number(n)) => {
                if let Some(port) = n.as_u64() {
                    if (1..=65535).contains(&port) {
                        CheckResult::Pass
                    } else {
                        CheckResult::Error(format!(
                            "Port at '{}' is {port}, expected 1–65535",
                            self.pointer
                        ))
                    }
                } else {
                    CheckResult::Error(format!(
                        "Port at '{}' is not a positive integer",
                        self.pointer
                    ))
                }
            }
            Some(_) => CheckResult::Error(format!("Value at '{}' is not a number", self.pointer)),
            None => CheckResult::Warning(format!("Port field '{}' is absent", self.pointer)),
        }
    }
}

/// Validates that a required field exists.
pub struct RequiredFieldCheck {
    pointer: String,
}

impl RequiredFieldCheck {
    #[must_use]
    pub fn new(json_pointer: &str) -> Self {
        Self {
            pointer: json_pointer.to_string(),
        }
    }
}

impl ConfigCheck for RequiredFieldCheck {
    fn name(&self) -> &str {
        "required_field"
    }

    fn check(&self, config: &Value) -> CheckResult {
        if config.pointer(&self.pointer).is_some() {
            CheckResult::Pass
        } else {
            CheckResult::Error(format!("Required field '{}' is missing", self.pointer))
        }
    }
}

/// Validates that a numeric field is within `[min, max]`.
pub struct NumericRangeCheck {
    pointer: String,
    min: f64,
    max: f64,
}

impl NumericRangeCheck {
    #[must_use]
    pub fn new(json_pointer: &str, min: f64, max: f64) -> Self {
        Self {
            pointer: json_pointer.to_string(),
            min,
            max,
        }
    }
}

impl ConfigCheck for NumericRangeCheck {
    fn name(&self) -> &str {
        "numeric_range"
    }

    fn check(&self, config: &Value) -> CheckResult {
        match config.pointer(&self.pointer) {
            Some(Value::Number(n)) => {
                let v = n.as_f64().unwrap_or(f64::NAN);
                if (self.min..=self.max).contains(&v) {
                    CheckResult::Pass
                } else {
                    CheckResult::Error(format!(
                        "Value at '{}' is {v}, expected [{}, {}]",
                        self.pointer, self.min, self.max
                    ))
                }
            }
            Some(_) => CheckResult::Error(format!("Value at '{}' is not a number", self.pointer)),
            None => CheckResult::Pass, // optional by default
        }
    }
}

/// Validates that a string field matches a regex pattern.
pub struct StringFormatCheck {
    pointer: String,
    pattern: regex::Regex,
    description: String,
}

impl StringFormatCheck {
    /// # Panics
    /// Panics if `pattern` is not a valid regex.
    #[must_use]
    pub fn new(json_pointer: &str, pattern: &str, description: &str) -> Self {
        Self {
            pointer: json_pointer.to_string(),
            pattern: regex::Regex::new(pattern).expect("invalid regex"),
            description: description.to_string(),
        }
    }
}

impl ConfigCheck for StringFormatCheck {
    fn name(&self) -> &str {
        "string_format"
    }

    fn check(&self, config: &Value) -> CheckResult {
        match config.pointer(&self.pointer) {
            Some(Value::String(s)) => {
                if self.pattern.is_match(s) {
                    CheckResult::Pass
                } else {
                    CheckResult::Error(format!(
                        "Value at '{}' does not match {}: '{s}'",
                        self.pointer, self.description
                    ))
                }
            }
            Some(_) => CheckResult::Error(format!("Value at '{}' is not a string", self.pointer)),
            None => CheckResult::Pass,
        }
    }
}

/// Validates that a string field looks like an existing file path.
pub struct FilePathCheck {
    pointer: String,
}

impl FilePathCheck {
    #[must_use]
    pub fn new(json_pointer: &str) -> Self {
        Self {
            pointer: json_pointer.to_string(),
        }
    }
}

impl ConfigCheck for FilePathCheck {
    fn name(&self) -> &str {
        "file_path"
    }

    fn check(&self, config: &Value) -> CheckResult {
        match config.pointer(&self.pointer) {
            Some(Value::String(s)) => {
                if std::path::Path::new(s).exists() {
                    CheckResult::Pass
                } else {
                    CheckResult::Warning(format!(
                        "Path at '{}' does not exist: '{s}'",
                        self.pointer
                    ))
                }
            }
            Some(_) => CheckResult::Error(format!("Value at '{}' is not a string", self.pointer)),
            None => CheckResult::Pass,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_validator_passes() {
        let v = ConfigValidator::new();
        let result = v.validate(&json!({}));
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn port_range_valid() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(PortRangeCheck::new("/port")));
        let result = v.validate(&json!({"port": 8080}));
        assert!(result.valid);
    }

    #[test]
    fn port_range_zero_invalid() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(PortRangeCheck::new("/port")));
        let result = v.validate(&json!({"port": 0}));
        assert!(!result.valid);
    }

    #[test]
    fn port_range_too_high() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(PortRangeCheck::new("/port")));
        let result = v.validate(&json!({"port": 70000}));
        assert!(!result.valid);
    }

    #[test]
    fn port_range_absent_warns() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(PortRangeCheck::new("/port")));
        let result = v.validate(&json!({}));
        assert!(result.valid); // warning, not error
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn required_field_present() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(RequiredFieldCheck::new("/name")));
        let result = v.validate(&json!({"name": "test"}));
        assert!(result.valid);
    }

    #[test]
    fn required_field_missing() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(RequiredFieldCheck::new("/name")));
        let result = v.validate(&json!({}));
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn numeric_range_in_bounds() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(NumericRangeCheck::new("/rate", 1.0, 1000.0)));
        let result = v.validate(&json!({"rate": 250}));
        assert!(result.valid);
    }

    #[test]
    fn numeric_range_out_of_bounds() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(NumericRangeCheck::new("/rate", 1.0, 1000.0)));
        let result = v.validate(&json!({"rate": 5000}));
        assert!(!result.valid);
    }

    #[test]
    fn string_format_matches() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(StringFormatCheck::new(
            "/version",
            r"^\d+\.\d+\.\d+$",
            "semver",
        )));
        let result = v.validate(&json!({"version": "1.2.3"}));
        assert!(result.valid);
    }

    #[test]
    fn string_format_rejects() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(StringFormatCheck::new(
            "/version",
            r"^\d+\.\d+\.\d+$",
            "semver",
        )));
        let result = v.validate(&json!({"version": "abc"}));
        assert!(!result.valid);
    }

    #[test]
    fn multiple_checks_collect_all_errors() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(RequiredFieldCheck::new("/a")));
        v.add_check(Box::new(RequiredFieldCheck::new("/b")));
        let result = v.validate(&json!({}));
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 2);
    }

    #[test]
    fn file_path_nonexistent_warns() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(FilePathCheck::new("/log_file")));
        let result = v.validate(&json!({"log_file": "/nonexistent/path/file.log"}));
        assert!(result.valid); // warning only
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn nested_pointer_works() {
        let mut v = ConfigValidator::new();
        v.add_check(Box::new(PortRangeCheck::new("/server/port")));
        let result = v.validate(&json!({"server": {"port": 443}}));
        assert!(result.valid);
    }
}
