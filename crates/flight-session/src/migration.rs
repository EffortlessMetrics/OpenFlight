// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! State format migration for session persistence.
//!
//! Tracks schema versions and provides a forward-only migration chain so that
//! persisted state written by older releases can be upgraded transparently on
//! load.

use serde_json::Value;

/// Current on-disk schema version.
pub const CURRENT_VERSION: u32 = 2;

/// Schema version tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StateVersion {
    /// V1: active_profile, device_assignments, last_sim only.
    V1 = 1,
    /// V2: adds window_positions, calibration_data, last_shutdown.
    V2 = 2,
}

impl StateVersion {
    /// Convert a raw integer to a known version, if valid.
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            1 => Some(Self::V1),
            2 => Some(Self::V2),
            _ => None,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Errors that can occur during state migration.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("unknown state version: {0}")]
    UnknownVersion(u32),

    #[error("deserialization failed: {0}")]
    Deserialization(#[from] serde_json::Error),

    #[error("migration from v{from} to v{to} failed: {reason}")]
    MigrationFailed { from: u32, to: u32, reason: String },
}

/// Migrate raw JSON state from `from_version` to [`CURRENT_VERSION`],
/// returning the deserialised [`SessionState`](crate::store::SessionState).
pub fn migrate(
    from_version: u32,
    state: Value,
) -> Result<crate::store::SessionState, MigrationError> {
    if from_version > CURRENT_VERSION {
        return Err(MigrationError::UnknownVersion(from_version));
    }

    let mut current = state;
    let mut version = from_version;

    while version < CURRENT_VERSION {
        current = match version {
            1 => migrate_v1_to_v2(current)?,
            _ => return Err(MigrationError::UnknownVersion(version)),
        };
        version += 1;
    }

    let session_state: crate::store::SessionState = serde_json::from_value(current)?;
    Ok(session_state)
}

// ── V1 → V2 ─────────────────────────────────────────────────────────────

/// Add `window_positions`, `calibration_data`, and `last_shutdown` fields
/// that were introduced in V2.
fn migrate_v1_to_v2(mut state: Value) -> Result<Value, MigrationError> {
    let obj = state
        .as_object_mut()
        .ok_or_else(|| MigrationError::MigrationFailed {
            from: 1,
            to: 2,
            reason: "state is not a JSON object".to_string(),
        })?;

    if !obj.contains_key("window_positions") {
        obj.insert(
            "window_positions".to_string(),
            Value::Object(serde_json::Map::new()),
        );
    }
    if !obj.contains_key("calibration_data") {
        obj.insert(
            "calibration_data".to_string(),
            Value::Object(serde_json::Map::new()),
        );
    }
    if !obj.contains_key("last_shutdown") {
        obj.insert("last_shutdown".to_string(), Value::Null);
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn version_enum_roundtrip() {
        assert_eq!(StateVersion::from_u32(1), Some(StateVersion::V1));
        assert_eq!(StateVersion::from_u32(2), Some(StateVersion::V2));
        assert_eq!(StateVersion::from_u32(99), None);
        assert_eq!(StateVersion::V2.as_u32(), 2);
    }

    #[test]
    fn migrate_v1_to_v2_adds_missing_fields() {
        let v1 = json!({
            "active_profile": "combat",
            "device_assignments": {"stick": "pitch_roll"},
            "last_sim": "MSFS"
        });

        let result = migrate(1, v1).unwrap();
        assert_eq!(result.active_profile.as_deref(), Some("combat"));
        assert!(result.window_positions.is_empty());
        assert!(result.calibration_data.is_empty());
        assert!(result.last_shutdown.is_none());
    }

    #[test]
    fn migrate_v2_is_identity() {
        let v2 = json!({
            "active_profile": "ga",
            "device_assignments": {},
            "last_sim": null,
            "window_positions": {},
            "calibration_data": {},
            "last_shutdown": null
        });

        let result = migrate(2, v2).unwrap();
        assert_eq!(result.active_profile.as_deref(), Some("ga"));
    }

    #[test]
    fn migrate_future_version_is_error() {
        let future = json!({"active_profile": null});
        let err = migrate(99, future).unwrap_err();
        assert!(matches!(err, MigrationError::UnknownVersion(99)));
    }

    #[test]
    fn migrate_v1_non_object_is_error() {
        let bad = json!("just a string");
        let err = migrate(1, bad).unwrap_err();
        assert!(matches!(
            err,
            MigrationError::MigrationFailed { from: 1, to: 2, .. }
        ));
    }

    #[test]
    fn migrate_v1_preserves_existing_v2_fields() {
        let v1_with_extras = json!({
            "active_profile": null,
            "device_assignments": {},
            "last_sim": null,
            "window_positions": {"main": {"x": 0, "y": 0, "width": 800, "height": 600}},
            "calibration_data": {},
            "last_shutdown": null
        });

        let result = migrate(1, v1_with_extras).unwrap();
        assert_eq!(result.window_positions.len(), 1);
    }

    #[test]
    fn migrate_corrupt_json_is_error() {
        // Valid V2 JSON object but with wrong types → deserialization error
        let bad_types = json!({
            "active_profile": 42,
            "device_assignments": "not a map",
            "last_sim": null,
            "window_positions": {},
            "calibration_data": {},
            "last_shutdown": null
        });

        let err = migrate(2, bad_types).unwrap_err();
        assert!(matches!(err, MigrationError::Deserialization(_)));
    }
}
