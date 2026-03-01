// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Sim variable lookup tables (ADR-002: Writers as Data)
//!
//! Variable tables are loaded from JSON diff tables and provide unified lookup
//! across MSFS, X-Plane, and DCS simulators with version-aware resolution.

use crate::types::SimulatorType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Data type for sim variables
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VarDataType {
    Float64,
    Float32,
    Int32,
    Bool,
    String,
}

/// A single sim variable mapping
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableMapping {
    pub sim_var_name: String,
    pub unit: String,
    pub data_type: VarDataType,
    pub settable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A versioned table of variable mappings for a specific sim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableTableData {
    pub sim: SimulatorType,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub variables: HashMap<String, VariableMapping>,
}

/// A diff entry indicating what changed between versions
#[derive(Debug, Clone, PartialEq)]
pub enum VariableChange {
    Added(VariableMapping),
    Removed(VariableMapping),
    Modified {
        old: VariableMapping,
        new: VariableMapping,
    },
}

/// Result of comparing two versions
#[derive(Debug, Clone)]
pub struct VersionDiff {
    pub sim: SimulatorType,
    pub from_version: String,
    pub to_version: String,
    pub changes: HashMap<String, VariableChange>,
}

/// Runtime variable table with multi-sim, multi-version lookup.
///
/// Tables are keyed by `(SimulatorType, version)`. A table with a
/// `base_version` inherits variables from its base, then applies its own
/// overrides on top.
#[derive(Debug, Clone)]
pub struct VariableTable {
    tables: HashMap<(SimulatorType, String), VariableTableData>,
}

impl Default for VariableTable {
    fn default() -> Self {
        Self::new()
    }
}

impl VariableTable {
    pub fn new() -> Self {
        Self {
            tables: HashMap::new(),
        }
    }

    /// Load a variable table from a JSON string.
    pub fn load_from_json(&mut self, json: &str) -> anyhow::Result<()> {
        let data: VariableTableData = serde_json::from_str(json)?;
        let key = (data.sim, data.version.clone());
        self.tables.insert(key, data);
        Ok(())
    }

    /// Load a variable table from a JSON file.
    pub fn load_from_file(&mut self, path: &Path) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(path)?;
        self.load_from_json(&content)
    }

    /// Resolve the full variable set for a `(sim, version)` pair.
    ///
    /// If the version declares a `base_version`, variables from the base are
    /// loaded first and then overridden by the version's own entries.
    fn resolve(&self, sim: SimulatorType, version: &str) -> HashMap<String, VariableMapping> {
        let key = (sim, version.to_string());
        let data = match self.tables.get(&key) {
            Some(d) => d,
            None => return HashMap::new(),
        };

        let mut resolved = if let Some(base) = &data.base_version {
            self.resolve(sim, base)
        } else {
            HashMap::new()
        };

        for (axis, mapping) in &data.variables {
            resolved.insert(axis.clone(), mapping.clone());
        }
        resolved
    }

    /// Look up a variable mapping for a specific sim, axis name, and version.
    pub fn lookup(&self, sim: SimulatorType, axis: &str, version: &str) -> Option<VariableMapping> {
        let resolved = self.resolve(sim, version);
        resolved.get(axis).cloned()
    }

    /// Compute the diff between two versions of the same simulator.
    pub fn version_diff(
        &self,
        sim: SimulatorType,
        from_version: &str,
        to_version: &str,
    ) -> VersionDiff {
        let from_vars = self.resolve(sim, from_version);
        let to_vars = self.resolve(sim, to_version);

        let mut changes = HashMap::new();

        // Detect added and modified
        for (axis, to_mapping) in &to_vars {
            match from_vars.get(axis) {
                None => {
                    changes.insert(axis.clone(), VariableChange::Added(to_mapping.clone()));
                }
                Some(from_mapping) => {
                    if from_mapping.sim_var_name != to_mapping.sim_var_name
                        || from_mapping.unit != to_mapping.unit
                        || from_mapping.data_type != to_mapping.data_type
                        || from_mapping.settable != to_mapping.settable
                        || from_mapping.min != to_mapping.min
                        || from_mapping.max != to_mapping.max
                    {
                        changes.insert(
                            axis.clone(),
                            VariableChange::Modified {
                                old: from_mapping.clone(),
                                new: to_mapping.clone(),
                            },
                        );
                    }
                }
            }
        }

        // Detect removed
        for (axis, from_mapping) in &from_vars {
            if !to_vars.contains_key(axis) {
                changes.insert(axis.clone(), VariableChange::Removed(from_mapping.clone()));
            }
        }

        VersionDiff {
            sim,
            from_version: from_version.to_string(),
            to_version: to_version.to_string(),
            changes,
        }
    }

    /// List all versions loaded for a simulator.
    pub fn list_versions(&self, sim: SimulatorType) -> Vec<&str> {
        self.tables
            .keys()
            .filter(|(s, _)| *s == sim)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// List all axis names available for a `(sim, version)`.
    pub fn list_axes(&self, sim: SimulatorType, version: &str) -> Vec<String> {
        let resolved = self.resolve(sim, version);
        let mut axes: Vec<String> = resolved.into_keys().collect();
        axes.sort();
        axes
    }

    /// Return all settable variables for a `(sim, version)`.
    pub fn settable_variables(
        &self,
        sim: SimulatorType,
        version: &str,
    ) -> HashMap<String, VariableMapping> {
        self.resolve(sim, version)
            .into_iter()
            .filter(|(_, m)| m.settable)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_msfs_2020_json() -> &'static str {
        include_str!("../writers/msfs_2020.json")
    }

    fn sample_msfs_2024_json() -> &'static str {
        include_str!("../writers/msfs_2024.json")
    }

    fn sample_xplane_json() -> &'static str {
        include_str!("../writers/xplane_12.json")
    }

    fn sample_dcs_json() -> &'static str {
        include_str!("../writers/dcs_world.json")
    }

    fn loaded_table() -> VariableTable {
        let mut table = VariableTable::new();
        table.load_from_json(sample_msfs_2020_json()).unwrap();
        table.load_from_json(sample_msfs_2024_json()).unwrap();
        table.load_from_json(sample_xplane_json()).unwrap();
        table.load_from_json(sample_dcs_json()).unwrap();
        table
    }

    // ── Basic loading ────────────────────────────────────────────

    #[test]
    fn load_msfs_2020() {
        let mut table = VariableTable::new();
        table.load_from_json(sample_msfs_2020_json()).unwrap();
        let versions = table.list_versions(SimulatorType::MSFS);
        assert!(versions.contains(&"2020"));
    }

    #[test]
    fn load_all_sims() {
        let table = loaded_table();
        assert!(!table.list_versions(SimulatorType::MSFS).is_empty());
        assert!(!table.list_versions(SimulatorType::XPlane).is_empty());
        assert!(!table.list_versions(SimulatorType::DCS).is_empty());
    }

    // ── Lookup ───────────────────────────────────────────────────

    #[test]
    fn lookup_msfs_aileron() {
        let table = loaded_table();
        let mapping = table
            .lookup(SimulatorType::MSFS, "aileron", "2020")
            .expect("aileron must exist");
        assert_eq!(mapping.sim_var_name, "AILERON POSITION");
        assert!(mapping.settable);
        assert_eq!(mapping.min, Some(-1.0));
        assert_eq!(mapping.max, Some(1.0));
    }

    #[test]
    fn lookup_xplane_throttle() {
        let table = loaded_table();
        let mapping = table
            .lookup(SimulatorType::XPlane, "throttle", "12")
            .expect("throttle must exist");
        assert_eq!(
            mapping.sim_var_name,
            "sim/cockpit2/engine/actuators/throttle_ratio[0]"
        );
        assert_eq!(mapping.data_type, VarDataType::Float32);
    }

    #[test]
    fn lookup_dcs_rudder() {
        let table = loaded_table();
        let mapping = table
            .lookup(SimulatorType::DCS, "rudder", "world")
            .expect("rudder must exist");
        assert_eq!(mapping.sim_var_name, "RUDDER");
    }

    #[test]
    fn lookup_missing_axis_returns_none() {
        let table = loaded_table();
        assert!(
            table
                .lookup(SimulatorType::MSFS, "nonexistent", "2020")
                .is_none()
        );
    }

    #[test]
    fn lookup_missing_version_returns_none() {
        let table = loaded_table();
        assert!(
            table
                .lookup(SimulatorType::MSFS, "aileron", "9999")
                .is_none()
        );
    }

    // ── Inheritance ──────────────────────────────────────────────

    #[test]
    fn msfs_2024_inherits_from_2020() {
        let table = loaded_table();
        // aileron is only in 2020 but should be accessible via 2024 (inheritance)
        let mapping = table
            .lookup(SimulatorType::MSFS, "aileron", "2024")
            .expect("aileron should be inherited from 2020");
        assert_eq!(mapping.sim_var_name, "AILERON POSITION");
    }

    #[test]
    fn msfs_2024_overrides_throttle() {
        let table = loaded_table();
        let mapping = table
            .lookup(SimulatorType::MSFS, "throttle", "2024")
            .expect("throttle must exist in 2024");
        // MSFS 2024 extends throttle min to -20 for reverse thrust
        assert_eq!(mapping.min, Some(-20.0));
    }

    #[test]
    fn msfs_2024_has_new_variables() {
        let table = loaded_table();
        assert!(
            table
                .lookup(SimulatorType::MSFS, "ap_speed", "2024")
                .is_some()
        );
        assert!(
            table
                .lookup(SimulatorType::MSFS, "ap_mach", "2024")
                .is_some()
        );
        // These should NOT exist in 2020
        assert!(
            table
                .lookup(SimulatorType::MSFS, "ap_speed", "2020")
                .is_none()
        );
    }

    // ── Version diff ─────────────────────────────────────────────

    #[test]
    fn version_diff_msfs_2020_to_2024() {
        let table = loaded_table();
        let diff = table.version_diff(SimulatorType::MSFS, "2020", "2024");

        assert_eq!(diff.sim, SimulatorType::MSFS);
        assert!(!diff.changes.is_empty());

        // ap_speed should be an addition
        assert!(matches!(
            diff.changes.get("ap_speed"),
            Some(VariableChange::Added(_))
        ));

        // throttle should be modified (min changed from 0 to -20)
        assert!(matches!(
            diff.changes.get("throttle"),
            Some(VariableChange::Modified { .. })
        ));
    }

    #[test]
    fn version_diff_same_version_is_empty() {
        let table = loaded_table();
        let diff = table.version_diff(SimulatorType::MSFS, "2020", "2020");
        assert!(diff.changes.is_empty());
    }

    // ── Axis listing ─────────────────────────────────────────────

    #[test]
    fn list_axes_msfs_2020() {
        let table = loaded_table();
        let axes = table.list_axes(SimulatorType::MSFS, "2020");
        assert!(axes.contains(&"aileron".to_string()));
        assert!(axes.contains(&"elevator".to_string()));
        assert!(axes.contains(&"throttle".to_string()));
    }

    #[test]
    fn list_axes_msfs_2024_includes_inherited() {
        let table = loaded_table();
        let axes = table.list_axes(SimulatorType::MSFS, "2024");
        // Should have 2020 axes plus 2024 additions
        assert!(axes.contains(&"aileron".to_string())); // inherited
        assert!(axes.contains(&"ap_speed".to_string())); // new in 2024
    }

    // ── Settable filter ──────────────────────────────────────────

    #[test]
    fn settable_variables_excludes_read_only() {
        let table = loaded_table();
        let settable = table.settable_variables(SimulatorType::MSFS, "2020");
        assert!(settable.contains_key("aileron"));
        assert!(!settable.contains_key("propeller")); // read-only
        assert!(!settable.contains_key("parking_brake")); // read-only
    }

    // ── Serialization round-trip ─────────────────────────────────

    #[test]
    fn variable_mapping_round_trip() {
        let mapping = VariableMapping {
            sim_var_name: "TEST VAR".to_string(),
            unit: "knots".to_string(),
            data_type: VarDataType::Float64,
            settable: true,
            min: Some(0.0),
            max: Some(500.0),
            description: Some("Test variable".to_string()),
        };
        let json = serde_json::to_string(&mapping).unwrap();
        let deserialized: VariableMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.sim_var_name, "TEST VAR");
        assert_eq!(deserialized.data_type, VarDataType::Float64);
    }

    #[test]
    fn variable_table_data_round_trip() {
        let json = sample_msfs_2020_json();
        let data: VariableTableData = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string_pretty(&data).unwrap();
        let deserialized: VariableTableData = serde_json::from_str(&serialized).unwrap();
        assert_eq!(data.sim, deserialized.sim);
        assert_eq!(data.version, deserialized.version);
        assert_eq!(data.variables.len(), deserialized.variables.len());
    }

    // ── Default trait ────────────────────────────────────────────

    #[test]
    fn default_table_is_empty() {
        let table = VariableTable::default();
        assert!(table.list_versions(SimulatorType::MSFS).is_empty());
    }
}
