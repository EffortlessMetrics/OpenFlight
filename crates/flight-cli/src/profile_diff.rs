// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile diff utility (REQ-676)

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

/// A diffable field category within a profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum ProfileField {
    /// Axis configuration (deadzones, expo, response curves).
    AxisConfig,
    /// Button-to-action mappings.
    ButtonMapping,
    /// Response curve parameters.
    CurveSettings,
    /// Deadzone sizes and shapes.
    DeadzoneSettings,
    /// Force feedback strength and effect tuning.
    FfbSettings,
    /// Profile name, description, and schema version.
    Metadata,
    /// Any field not covered by the predefined categories.
    Other(String),
}

impl std::fmt::Display for ProfileField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileField::AxisConfig => write!(f, "axis_config"),
            ProfileField::ButtonMapping => write!(f, "button_mapping"),
            ProfileField::CurveSettings => write!(f, "curve_settings"),
            ProfileField::DeadzoneSettings => write!(f, "deadzone_settings"),
            ProfileField::FfbSettings => write!(f, "ffb_settings"),
            ProfileField::Metadata => write!(f, "metadata"),
            ProfileField::Other(s) => write!(f, "{}", s),
        }
    }
}

/// A single difference between two profiles.
/// A single difference between two profiles.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct DiffEntry {
    /// The field that differs.
    pub field: String,
    /// The value in profile A (None if field was added).
    pub old_value: Option<String>,
    /// The value in profile B (None if field was removed).
    pub new_value: Option<String>,
}

/// Diff two profile files (JSON) and return field-level differences.
pub fn diff_profiles(a_path: &Path, b_path: &Path) -> Vec<DiffEntry> {
    let a_content = match std::fs::read_to_string(a_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let b_content = match std::fs::read_to_string(b_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let a_val: Value = match serde_json::from_str(&a_content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let b_val: Value = match serde_json::from_str(&b_content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    diff_values(&a_val, &b_val, "")
}

fn diff_values(a: &Value, b: &Value, prefix: &str) -> Vec<DiffEntry> {
    if a == b {
        return vec![];
    }

    match (a, b) {
        (Value::Object(a_map), Value::Object(b_map)) => {
            let mut entries = Vec::new();
            let all_keys: BTreeSet<&String> = a_map.keys().chain(b_map.keys()).collect();

            for key in all_keys {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{}.{}", prefix, key)
                };

                match (a_map.get(key), b_map.get(key)) {
                    (Some(av), Some(bv)) => {
                        entries.extend(diff_values(av, bv, &path));
                    }
                    (Some(av), None) => {
                        entries.push(DiffEntry {
                            field: path,
                            old_value: Some(compact_json(av)),
                            new_value: None,
                        });
                    }
                    (None, Some(bv)) => {
                        entries.push(DiffEntry {
                            field: path,
                            old_value: None,
                            new_value: Some(compact_json(bv)),
                        });
                    }
                    (None, None) => unreachable!(),
                }
            }
            entries
        }
        _ => {
            vec![DiffEntry {
                field: prefix.to_string(),
                old_value: Some(compact_json(a)),
                new_value: Some(compact_json(b)),
            }]
        }
    }
}

fn compact_json(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| v.to_string())
}

/// Format diff entries for human-readable output.
pub fn format_diff_human(entries: &[DiffEntry]) -> String {
    if entries.is_empty() {
        return "Profiles are identical.".to_string();
    }

    let mut lines = Vec::with_capacity(entries.len());
    for e in entries {
        let old = e.old_value.as_deref().unwrap_or("(absent)");
        let new = e.new_value.as_deref().unwrap_or("(removed)");
        lines.push(format!("  {}: {} -> {}", e.field, old, new));
    }
    lines.join("\n")
}

/// Format diff entries as JSON.
pub fn format_diff_json(entries: &[DiffEntry]) -> String {
    serde_json::to_string_pretty(entries).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_profile(name: &str, content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("flight_cli_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn identical_profiles_produce_empty_diff() {
        let content = r#"{"axis_config": {"deadzone": 0.05}, "name": "test"}"#;
        let a = write_temp_profile("ident_a.json", content);
        let b = write_temp_profile("ident_b.json", content);
        let diff = diff_profiles(&a, &b);
        assert!(diff.is_empty());
    }

    #[test]
    fn axis_config_change_detected() {
        let a = write_temp_profile("axis_a.json", r#"{"axis_config": {"deadzone": 0.05}}"#);
        let b = write_temp_profile("axis_b.json", r#"{"axis_config": {"deadzone": 0.10}}"#);
        let diff = diff_profiles(&a, &b);
        assert!(!diff.is_empty());
        assert!(diff.iter().any(|e| e.field.contains("deadzone")));
    }

    #[test]
    fn field_added_shows_none_old_value() {
        let a = write_temp_profile("add_a.json", r#"{"name": "base"}"#);
        let b = write_temp_profile("add_b.json", r#"{"name": "base", "new_field": 42}"#);
        let diff = diff_profiles(&a, &b);
        let added = diff.iter().find(|e| e.field == "new_field").unwrap();
        assert!(added.old_value.is_none());
        assert!(added.new_value.is_some());
    }

    #[test]
    fn field_removed_shows_none_new_value() {
        let a = write_temp_profile("rem_a.json", r#"{"name": "base", "old_field": true}"#);
        let b = write_temp_profile("rem_b.json", r#"{"name": "base"}"#);
        let diff = diff_profiles(&a, &b);
        let removed = diff.iter().find(|e| e.field == "old_field").unwrap();
        assert!(removed.old_value.is_some());
        assert!(removed.new_value.is_none());
    }

    #[test]
    fn human_format_identical_says_so() {
        let output = format_diff_human(&[]);
        assert_eq!(output, "Profiles are identical.");
    }

    #[test]
    fn json_format_is_valid() {
        let entries = vec![DiffEntry {
            field: "test".to_string(),
            old_value: Some("1".to_string()),
            new_value: Some("2".to_string()),
        }];
        let output = format_diff_json(&entries);
        let parsed: Vec<DiffEntry> = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.len(), 1);
    }
}
