// SPDX-License-Identifier: MIT OR Apache-2.0

//! Schema validation module for YAML and JSON files.
//!
//! This module provides functionality to validate YAML files against JSON Schema
//! definitions, with detailed error reporting including line numbers and suggestions.

use anyhow::Result;
use jsonschema::Validator;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::path::Path;

/// Error information for schema validation failures.
#[derive(Debug, Clone)]
pub struct SchemaError {
    /// Error code in format INF-SCHEMA-NNN
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// File path where the error occurred
    pub file_path: String,
    /// Line number (if available)
    pub line: Option<usize>,
    /// Column number (if available)
    pub column: Option<usize>,
    /// Expected value or format
    pub expected: Option<String>,
    /// Actual value found
    pub found: Option<String>,
    /// Suggestion for fixing the error
    pub suggestion: Option<String>,
}

impl SchemaError {
    /// Format the error for display according to INF-SCHEMA-NNN format.
    pub fn format(&self) -> String {
        let mut output = format!("[ERROR] {}: {}", self.code, self.message);

        // Add file location
        if let (Some(line), Some(column)) = (self.line, self.column) {
            output.push_str(&format!("\n  File: {}:{}:{}", self.file_path, line, column));
        } else if let Some(line) = self.line {
            output.push_str(&format!("\n  File: {}:{}", self.file_path, line));
        } else {
            output.push_str(&format!("\n  File: {}", self.file_path));
        }

        // Add expected/found information
        if let Some(expected) = &self.expected {
            output.push_str(&format!("\n  Expected: {}", expected));
        }
        if let Some(found) = &self.found {
            output.push_str(&format!("\n  Found: {}", found));
        }

        // Add suggestion
        if let Some(suggestion) = &self.suggestion {
            output.push_str(&format!("\n  Suggestion: {}", suggestion));
        }

        output
    }
}

/// Validate a YAML file against a JSON Schema.
///
/// This function loads the YAML file, converts it to JSON for validation,
/// and reports any schema violations with line numbers and suggestions.
///
/// # Arguments
///
/// * `yaml_path` - Path to the YAML file to validate
/// * `schema_path` - Path to the JSON Schema file
///
/// # Returns
///
/// Returns `Ok(())` if validation succeeds, or `Err` containing a vector
/// of `SchemaError` instances describing all validation failures.
///
/// # Errors
///
/// Returns an error if:
/// - The YAML file cannot be read or parsed
/// - The schema file cannot be read or parsed
/// - The schema is invalid
/// - The YAML content violates the schema
pub fn validate_yaml_against_schema(
    yaml_path: &Path,
    schema_path: &Path,
) -> Result<(), Vec<SchemaError>> {
    // Read the original YAML text for line number reporting
    let yaml_text = std::fs::read_to_string(yaml_path).map_err(|e| {
        vec![SchemaError {
            code: "INF-SCHEMA-001".to_string(),
            message: format!("Failed to read YAML file: {}", e),
            file_path: yaml_path.display().to_string(),
            line: None,
            column: None,
            expected: None,
            found: None,
            suggestion: Some("Ensure the file exists and is readable".to_string()),
        }]
    })?;

    // Parse YAML into serde_yaml::Value
    let yaml_value: YamlValue = serde_yaml::from_str(&yaml_text).map_err(|e| {
        vec![SchemaError {
            code: "INF-SCHEMA-002".to_string(),
            message: format!("Invalid YAML syntax: {}", e),
            file_path: yaml_path.display().to_string(),
            line: e.location().map(|loc| loc.line()),
            column: e.location().map(|loc| loc.column()),
            expected: Some("Valid YAML syntax".to_string()),
            found: Some("Malformed YAML".to_string()),
            suggestion: Some("Check YAML syntax, indentation, and special characters".to_string()),
        }]
    })?;

    // Convert YAML to JSON for jsonschema validation
    let json_value: JsonValue = serde_json::to_value(&yaml_value).map_err(|e| {
        vec![SchemaError {
            code: "INF-SCHEMA-003".to_string(),
            message: format!("Failed to convert YAML to JSON: {}", e),
            file_path: yaml_path.display().to_string(),
            line: None,
            column: None,
            expected: None,
            found: None,
            suggestion: Some("Ensure YAML contains only JSON-compatible types".to_string()),
        }]
    })?;

    // Read and parse the schema
    let schema_text = std::fs::read_to_string(schema_path).map_err(|e| {
        vec![SchemaError {
            code: "INF-SCHEMA-004".to_string(),
            message: format!("Failed to read schema file: {}", e),
            file_path: schema_path.display().to_string(),
            line: None,
            column: None,
            expected: None,
            found: None,
            suggestion: Some("Ensure the schema file exists and is readable".to_string()),
        }]
    })?;

    let schema_json: JsonValue = serde_json::from_str(&schema_text).map_err(|e| {
        vec![SchemaError {
            code: "INF-SCHEMA-005".to_string(),
            message: format!("Invalid JSON schema: {}", e),
            file_path: schema_path.display().to_string(),
            line: None,
            column: None,
            expected: Some("Valid JSON".to_string()),
            found: Some("Malformed JSON".to_string()),
            suggestion: Some("Check JSON syntax in schema file".to_string()),
        }]
    })?;

    // Compile the schema
    let compiled_schema = Validator::new(&schema_json).map_err(|e| {
        vec![SchemaError {
            code: "INF-SCHEMA-006".to_string(),
            message: format!("Failed to compile schema: {}", e),
            file_path: schema_path.display().to_string(),
            line: None,
            column: None,
            expected: Some("Valid JSON Schema Draft 7".to_string()),
            found: Some("Invalid schema definition".to_string()),
            suggestion: Some("Ensure schema follows JSON Schema Draft 7 specification".to_string()),
        }]
    })?;

    // Validate the JSON against the schema
    if compiled_schema.is_valid(&json_value) {
        return Ok(());
    }

    // If validation failed, collect all errors
    let validation_errors: Vec<_> = compiled_schema.iter_errors(&json_value).collect();

    if validation_errors.is_empty() {
        return Ok(());
    }

    let schema_errors: Vec<SchemaError> = validation_errors
        .into_iter()
        .enumerate()
        .map(|(idx, error)| {
            let instance_path = error.instance_path.to_string();
            let schema_path_str = error.schema_path.to_string();

            // Try to extract line number from instance path
            let line = estimate_line_number(&yaml_text, &instance_path);

            // Generate error code based on index
            let code = format!("INF-SCHEMA-{:03}", 100 + idx);

            // Create detailed error message
            let message = format!("{}", error);

            // Generate expected/found/suggestion based on error type
            let (expected, found, suggestion) =
                generate_error_details(&error.to_string(), &instance_path, &schema_path_str);

            SchemaError {
                code,
                message,
                file_path: yaml_path.display().to_string(),
                line,
                column: None,
                expected: Some(expected),
                found: Some(found),
                suggestion: Some(suggestion),
            }
        })
        .collect();

    Err(schema_errors)
}

/// Estimate the line number in YAML text based on JSON pointer path.
///
/// This is a best-effort approach since we lose exact line information
/// when converting YAML to JSON.
fn estimate_line_number(yaml_text: &str, instance_path: &str) -> Option<usize> {
    if instance_path.is_empty() || instance_path == "/" {
        return Some(1);
    }

    // Extract the last component of the path
    let path_parts: Vec<&str> = instance_path.split('/').filter(|s| !s.is_empty()).collect();

    if path_parts.is_empty() {
        return Some(1);
    }

    // Search for the field name in the YAML text
    let search_term = path_parts.last().unwrap();

    for (line_num, line) in yaml_text.lines().enumerate() {
        if line.contains(search_term) {
            return Some(line_num + 1);
        }
    }

    // If we can't find it, return None
    None
}

/// Generate detailed error information based on the validation error message.
fn generate_error_details(
    error_msg: &str,
    instance_path: &str,
    _schema_path: &str,
) -> (String, String, String) {
    let error_lower = error_msg.to_lowercase();

    // Handle missing required properties
    if error_lower.contains("required") && error_lower.contains("missing") {
        let expected = "All required fields must be present".to_string();
        let found = "One or more required fields are missing".to_string();
        let suggestion = format!(
            "Add the missing required field(s) at path '{}'",
            instance_path
        );
        return (expected, found, suggestion);
    }

    // Handle additional properties
    if error_lower.contains("additional properties") {
        let expected = "Only defined properties are allowed".to_string();
        let found = "Unexpected additional property".to_string();
        let suggestion = "Remove the additional property or update the schema".to_string();
        return (expected, found, suggestion);
    }

    // Handle pattern mismatches
    if error_lower.contains("pattern") {
        let expected = "Value matching the required pattern".to_string();
        let found = "Value does not match pattern".to_string();
        let suggestion = format!(
            "Ensure the value at '{}' matches the required format",
            instance_path
        );
        return (expected, found, suggestion);
    }

    // Handle enum mismatches
    if error_lower.contains("enum") {
        let expected = "One of the allowed enum values".to_string();
        let found = "Value not in allowed set".to_string();
        let suggestion = "Use one of the allowed values defined in the schema".to_string();
        return (expected, found, suggestion);
    }

    // Handle type mismatches
    if error_lower.contains("type") {
        let expected = "Correct data type".to_string();
        let found = "Incorrect data type".to_string();
        let suggestion = format!(
            "Ensure the value at '{}' has the correct type",
            instance_path
        );
        return (expected, found, suggestion);
    }

    // Handle minProperties
    if error_lower.contains("minproperties") {
        let expected = "Object with at least one property".to_string();
        let found = "Empty object".to_string();
        let suggestion = "Add at least one property to the object".to_string();
        return (expected, found, suggestion);
    }

    // Default case
    (
        "Valid value according to schema".to_string(),
        "Invalid value".to_string(),
        format!("Check the schema requirements for path '{}'", instance_path),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    fn schemas_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("schemas")
    }

    #[test]
    fn test_valid_spec_ledger() {
        let yaml_path = fixtures_dir().join("minimal/specs/spec_ledger.yaml");
        let schema_path = schemas_dir().join("spec_ledger.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_ok(), "Valid spec ledger should pass validation");
    }

    #[test]
    fn test_spec_ledger_missing_required() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/spec_ledger_missing_required.yaml");
        let schema_path = schemas_dir().join("spec_ledger.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Missing required field should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
            let error = &errors[0];
            assert!(error.code.starts_with("INF-SCHEMA-"));
            assert!(error.message.to_lowercase().contains("required"));
        }
    }

    #[test]
    fn test_spec_ledger_invalid_id_pattern() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/spec_ledger_invalid_id_pattern.yaml");
        let schema_path = schemas_dir().join("spec_ledger.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Invalid ID pattern should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
            let error = &errors[0];
            assert!(error.code.starts_with("INF-SCHEMA-"));
        }
    }

    #[test]
    fn test_spec_ledger_additional_properties() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/spec_ledger_additional_properties.yaml");
        let schema_path = schemas_dir().join("spec_ledger.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Additional properties should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
            let error = &errors[0];
            assert!(error.code.starts_with("INF-SCHEMA-"));
            assert!(error.message.to_lowercase().contains("additional"));
        }
    }

    #[test]
    fn test_spec_ledger_empty_test_object() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/spec_ledger_empty_test_object.yaml");
        let schema_path = schemas_dir().join("spec_ledger.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(
            result.is_err(),
            "Empty test object should fail minProperties"
        );

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
        }
    }

    #[test]
    fn test_front_matter_missing_links() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/front_matter_missing_links.yaml");
        let schema_path = schemas_dir().join("front_matter.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Missing required links field should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
            let error = &errors[0];
            assert!(error.code.starts_with("INF-SCHEMA-"));
        }
    }

    #[test]
    fn test_front_matter_invalid_kind() {
        let yaml_path = fixtures_dir().join("invalid/schema_errors/front_matter_invalid_kind.yaml");
        let schema_path = schemas_dir().join("front_matter.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Invalid kind enum value should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
        }
    }

    #[test]
    fn test_front_matter_additional_properties() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/front_matter_additional_properties.yaml");
        let schema_path = schemas_dir().join("front_matter.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Additional properties should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
        }
    }

    #[test]
    fn test_invariants_missing_rust_version() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/invariants_missing_rust_version.yaml");
        let schema_path = schemas_dir().join("invariants.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Missing required rust_version should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
        }
    }

    #[test]
    fn test_invariants_invalid_port_name() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/invariants_invalid_port_name.yaml");
        let schema_path = schemas_dir().join("invariants.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Invalid port name pattern should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
        }
    }

    #[test]
    fn test_invariants_invalid_env_var_name() {
        let yaml_path =
            fixtures_dir().join("invalid/schema_errors/invariants_invalid_env_var_name.yaml");
        let schema_path = schemas_dir().join("invariants.schema.json");

        let result = validate_yaml_against_schema(&yaml_path, &schema_path);
        assert!(result.is_err(), "Invalid env var name pattern should fail");

        if let Err(errors) = result {
            assert!(!errors.is_empty(), "Should have at least one error");
        }
    }

    #[test]
    fn test_error_formatting() {
        let error = SchemaError {
            code: "INF-SCHEMA-100".to_string(),
            message: "Missing required field 'status'".to_string(),
            file_path: "specs/spec_ledger.yaml".to_string(),
            line: Some(15),
            column: Some(3),
            expected: Some(
                "status field with value 'draft', 'implemented', 'tested', or 'deprecated'"
                    .to_string(),
            ),
            found: Some("(field missing)".to_string()),
            suggestion: Some("Add 'status: draft' to requirement REQ-3".to_string()),
        };

        let formatted = error.format();
        assert!(formatted.contains("[ERROR] INF-SCHEMA-100"));
        assert!(formatted.contains("File: specs/spec_ledger.yaml:15:3"));
        assert!(formatted.contains("Expected:"));
        assert!(formatted.contains("Found:"));
        assert!(formatted.contains("Suggestion:"));
    }
}
