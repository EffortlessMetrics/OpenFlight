// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile migration system for upgrading between schema versions.
//!
//! Supports chaining migrations (e.g. v1→v2→v3) and custom migration registration.

use serde_json::Value;
use std::fmt;

/// Error type for profile migrations.
#[derive(Debug)]
pub enum MigrationError {
    /// The source or target version is not recognised.
    UnsupportedVersion(String),
    /// The migration would lose data that cannot be recovered.
    DataLoss(String),
    /// The input value does not match the expected schema.
    InvalidSchema(String),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported version: {v}"),
            Self::DataLoss(msg) => write!(f, "data loss: {msg}"),
            Self::InvalidSchema(msg) => write!(f, "invalid schema: {msg}"),
        }
    }
}

impl std::error::Error for MigrationError {}

/// A single migration step between two adjacent schema versions.
pub struct ProfileMigration {
    pub from_version: &'static str,
    pub to_version: &'static str,
    pub description: &'static str,
    pub migrate_fn: fn(Value) -> Result<Value, MigrationError>,
}

/// Registry that holds an ordered list of migrations and can chain them.
pub struct MigrationRegistry {
    migrations: Vec<ProfileMigration>,
}

impl MigrationRegistry {
    /// Create a new registry pre-populated with the built-in v1→v2 and v2→v3 migrations.
    pub fn new() -> Self {
        let mut registry = Self {
            migrations: Vec::new(),
        };
        registry.migrations.push(ProfileMigration {
            from_version: "v1",
            to_version: "v2",
            description: "Add sensitivity field to axes (default 1.0)",
            migrate_fn: migrate_v1_to_v2,
        });
        registry.migrations.push(ProfileMigration {
            from_version: "v2",
            to_version: "v3",
            description: "Rename expo to exponential, add response_curve_type",
            migrate_fn: migrate_v2_to_v3,
        });
        registry
    }

    /// Register an additional custom migration.
    pub fn register(&mut self, migration: ProfileMigration) {
        self.migrations.push(migration);
    }

    /// Return all distinct version strings that appear in the registry.
    pub fn available_versions(&self) -> Vec<&str> {
        let mut versions: Vec<&str> = Vec::new();
        for m in &self.migrations {
            if !versions.contains(&m.from_version) {
                versions.push(m.from_version);
            }
            if !versions.contains(&m.to_version) {
                versions.push(m.to_version);
            }
        }
        versions
    }

    /// Check whether a migration path exists from `from` to `to`.
    pub fn can_migrate(&self, from: &str, to: &str) -> bool {
        self.build_path(from, to).is_some()
    }

    /// Migrate a JSON value from version `from` to version `to`, chaining as needed.
    pub fn migrate(&self, value: Value, from: &str, to: &str) -> Result<Value, MigrationError> {
        if from == to {
            return Ok(value);
        }

        let path = self.build_path(from, to).ok_or_else(|| {
            MigrationError::UnsupportedVersion(format!("no migration path from {from} to {to}"))
        })?;

        let mut current = value;
        for migration in path {
            current = (migration.migrate_fn)(current)?;
        }
        Ok(current)
    }

    /// Build an ordered chain of migration steps from `from` to `to`.
    fn build_path(&self, from: &str, to: &str) -> Option<Vec<&ProfileMigration>> {
        let mut path = Vec::new();
        let mut current = from;
        while current != to {
            let step = self.migrations.iter().find(|m| m.from_version == current)?;
            path.push(step);
            current = step.to_version;
        }
        Some(path)
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in migrations
// ---------------------------------------------------------------------------

/// v1→v2: Add `sensitivity` field (default 1.0) to every axis entry.
fn migrate_v1_to_v2(mut value: Value) -> Result<Value, MigrationError> {
    let axes = value
        .get_mut("axes")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| MigrationError::InvalidSchema("missing or invalid 'axes' object".into()))?;

    for (_name, axis) in axes.iter_mut() {
        let obj = axis
            .as_object_mut()
            .ok_or_else(|| MigrationError::InvalidSchema("axis entry is not an object".into()))?;
        obj.entry("sensitivity").or_insert(Value::from(1.0));
    }

    if let Some(obj) = value.as_object_mut() {
        obj.insert("schema_version".to_string(), Value::from("v2"));
    }

    Ok(value)
}

/// v2→v3: Rename `expo` to `exponential` and add `response_curve_type` field.
fn migrate_v2_to_v3(mut value: Value) -> Result<Value, MigrationError> {
    let axes = value
        .get_mut("axes")
        .and_then(|v| v.as_object_mut())
        .ok_or_else(|| MigrationError::InvalidSchema("missing or invalid 'axes' object".into()))?;

    for (_name, axis) in axes.iter_mut() {
        let obj = axis
            .as_object_mut()
            .ok_or_else(|| MigrationError::InvalidSchema("axis entry is not an object".into()))?;

        if let Some(expo_val) = obj.remove("expo") {
            obj.insert("exponential".to_string(), expo_val);
        }

        obj.entry("response_curve_type")
            .or_insert(Value::from("default"));
    }

    if let Some(obj) = value.as_object_mut() {
        obj.insert("schema_version".to_string(), Value::from("v3"));
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_v1() -> Value {
        json!({
            "schema_version": "v1",
            "axes": {
                "pitch": { "deadzone": 0.03, "expo": 0.2 },
                "roll":  { "deadzone": 0.05, "expo": 0.3 }
            }
        })
    }

    #[test]
    fn migrate_v1_to_v2_adds_sensitivity() {
        let reg = MigrationRegistry::new();
        let result = reg.migrate(sample_v1(), "v1", "v2").unwrap();
        let pitch = &result["axes"]["pitch"];
        assert_eq!(pitch["sensitivity"], json!(1.0));
        assert_eq!(pitch["expo"], json!(0.2)); // preserved
    }

    #[test]
    fn migrate_v2_to_v3_renames_expo() {
        let reg = MigrationRegistry::new();
        let v2 = reg.migrate(sample_v1(), "v1", "v2").unwrap();
        let v3 = reg.migrate(v2, "v2", "v3").unwrap();

        let pitch = &v3["axes"]["pitch"];
        assert!(pitch.get("expo").is_none());
        assert_eq!(pitch["exponential"], json!(0.2));
        assert_eq!(pitch["response_curve_type"], json!("default"));
    }

    #[test]
    fn chain_v1_to_v3() {
        let reg = MigrationRegistry::new();
        let result = reg.migrate(sample_v1(), "v1", "v3").unwrap();

        let roll = &result["axes"]["roll"];
        assert_eq!(roll["sensitivity"], json!(1.0));
        assert_eq!(roll["exponential"], json!(0.3));
        assert_eq!(roll["response_curve_type"], json!("default"));
        assert!(roll.get("expo").is_none());
    }

    #[test]
    fn same_version_is_noop() {
        let reg = MigrationRegistry::new();
        let input = sample_v1();
        let result = reg.migrate(input.clone(), "v1", "v1").unwrap();
        assert_eq!(input, result);
    }

    #[test]
    fn unknown_source_version_errors() {
        let reg = MigrationRegistry::new();
        let result = reg.migrate(sample_v1(), "v0", "v2");
        assert!(matches!(result, Err(MigrationError::UnsupportedVersion(_))));
    }

    #[test]
    fn unknown_target_version_errors() {
        let reg = MigrationRegistry::new();
        let result = reg.migrate(sample_v1(), "v1", "v99");
        assert!(matches!(result, Err(MigrationError::UnsupportedVersion(_))));
    }

    #[test]
    fn can_migrate_true_for_valid_path() {
        let reg = MigrationRegistry::new();
        assert!(reg.can_migrate("v1", "v2"));
        assert!(reg.can_migrate("v1", "v3"));
        assert!(reg.can_migrate("v2", "v3"));
    }

    #[test]
    fn can_migrate_false_for_invalid_path() {
        let reg = MigrationRegistry::new();
        assert!(!reg.can_migrate("v3", "v1")); // no downgrade
        assert!(!reg.can_migrate("v0", "v2")); // unknown
    }

    #[test]
    fn available_versions_includes_all() {
        let reg = MigrationRegistry::new();
        let versions = reg.available_versions();
        assert!(versions.contains(&"v1"));
        assert!(versions.contains(&"v2"));
        assert!(versions.contains(&"v3"));
    }

    #[test]
    fn data_preservation_through_chain() {
        let input = json!({
            "schema_version": "v1",
            "sim": "msfs",
            "aircraft": { "icao": "C172" },
            "axes": {
                "pitch": { "deadzone": 0.03, "expo": 0.2, "slew_rate": 1.5 }
            }
        });
        let reg = MigrationRegistry::new();
        let result = reg.migrate(input, "v1", "v3").unwrap();

        assert_eq!(result["sim"], json!("msfs"));
        assert_eq!(result["aircraft"]["icao"], json!("C172"));
        assert_eq!(result["axes"]["pitch"]["deadzone"], json!(0.03));
        assert_eq!(result["axes"]["pitch"]["slew_rate"], json!(1.5));
    }

    #[test]
    fn invalid_schema_missing_axes() {
        let reg = MigrationRegistry::new();
        let bad = json!({ "schema_version": "v1" });
        let result = reg.migrate(bad, "v1", "v2");
        assert!(matches!(result, Err(MigrationError::InvalidSchema(_))));
    }

    #[test]
    fn custom_migration_registration() {
        let mut reg = MigrationRegistry::new();
        reg.register(ProfileMigration {
            from_version: "v3",
            to_version: "v4",
            description: "Add custom field",
            migrate_fn: |mut v| {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("custom".to_string(), Value::from(true));
                    obj.insert("schema_version".to_string(), Value::from("v4"));
                }
                Ok(v)
            },
        });
        assert!(reg.can_migrate("v1", "v4"));
        let result = reg.migrate(sample_v1(), "v1", "v4").unwrap();
        assert_eq!(result["custom"], json!(true));
    }

    #[test]
    fn v2_to_v3_without_expo_field() {
        let input = json!({
            "schema_version": "v2",
            "axes": {
                "throttle": { "deadzone": 0.01, "sensitivity": 1.0 }
            }
        });
        let reg = MigrationRegistry::new();
        let result = reg.migrate(input, "v2", "v3").unwrap();
        let throttle = &result["axes"]["throttle"];
        assert!(throttle.get("expo").is_none());
        assert!(throttle.get("exponential").is_none());
        assert_eq!(throttle["response_curve_type"], json!("default"));
    }
}
