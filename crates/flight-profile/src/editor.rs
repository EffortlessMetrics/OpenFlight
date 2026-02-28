// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Profile editing operations.
//!
//! [`ProfileEditor`] wraps a [`Profile`] and provides a fluent API for
//! modifying axis configurations, sim-specific overrides, and merging.
//! [`ProfileDiff`] captures the differences between two profiles as a
//! structured list of [`DiffChange`] entries.

use crate::{AxisConfig, CurvePoint, PROFILE_SCHEMA_VERSION, Profile, Result};
use std::collections::HashMap;
use std::fmt;

// ── ProfileEditor ────────────────────────────────────────────────────────────

/// Mutable wrapper around a [`Profile`] that exposes editing helpers.
#[derive(Debug, Clone)]
pub struct ProfileEditor {
    profile: Profile,
    /// Opaque sim-specific key-value overrides keyed by `(sim, key)`.
    sim_overrides: HashMap<String, HashMap<String, String>>,
}

impl ProfileEditor {
    /// Create an editor for an existing profile.
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            sim_overrides: HashMap::new(),
        }
    }

    /// Create an editor from an empty default profile.
    pub fn empty() -> Self {
        Self::new(Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: None,
            aircraft: None,
            axes: HashMap::new(),
            pof_overrides: None,
        })
    }

    /// Return a reference to the inner profile.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// Consume the editor and return the inner profile.
    pub fn into_profile(self) -> Profile {
        self.profile
    }

    /// Return a reference to the sim-specific overrides.
    pub fn sim_overrides(&self) -> &HashMap<String, HashMap<String, String>> {
        &self.sim_overrides
    }

    // ── axis operations ──────────────────────────────────────────────────

    /// Add or replace an axis configuration.
    pub fn set_axis(&mut self, name: impl Into<String>, config: AxisConfig) -> &mut Self {
        self.profile.axes.insert(name.into(), config);
        self
    }

    /// Remove an axis by name. Returns `true` if the axis existed.
    pub fn remove_axis(&mut self, name: &str) -> bool {
        self.profile.axes.remove(name).is_some()
    }

    /// Set the deadzone for an axis, creating a default axis entry if needed.
    pub fn set_deadzone(&mut self, axis: &str, value: f32) -> &mut Self {
        let config = self
            .profile
            .axes
            .entry(axis.to_owned())
            .or_insert_with(AxisConfig::default_empty);
        config.deadzone = Some(value);
        self
    }

    /// Set the response curve for an axis.
    pub fn set_curve(
        &mut self,
        axis: &str,
        _curve_type: &str,
        points: Vec<CurvePoint>,
    ) -> &mut Self {
        let config = self
            .profile
            .axes
            .entry(axis.to_owned())
            .or_insert_with(AxisConfig::default_empty);
        config.curve = Some(points);
        self
    }

    /// Set a sim-specific override value.
    pub fn set_sim_specific(
        &mut self,
        sim: impl Into<String>,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self {
        self.sim_overrides
            .entry(sim.into())
            .or_default()
            .insert(key.into(), value.into());
        self
    }

    /// Merge another profile's settings into this one (last-writer-wins).
    pub fn merge_from(&mut self, other: &Profile) -> Result<&mut Self> {
        self.profile = self.profile.merge_with(other)?;
        Ok(self)
    }

    /// Compute a structured diff between the inner profile and `other`.
    pub fn diff(&self, other: &Profile) -> ProfileDiff {
        ProfileDiff::compute(&self.profile, other)
    }
}

// ── AxisConfig helper ────────────────────────────────────────────────────────

impl AxisConfig {
    /// An axis configuration with all fields set to `None`/empty.
    pub fn default_empty() -> Self {
        Self {
            deadzone: None,
            expo: None,
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        }
    }
}

// ── ProfileDiff ──────────────────────────────────────────────────────────────

/// The kind of change detected between two profiles.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffChange {
    /// A key present only in the right (other) profile.
    Added { section: String, detail: String },
    /// A key present only in the left (base) profile.
    Removed { section: String, detail: String },
    /// A key whose value differs between the two profiles.
    Modified {
        section: String,
        detail: String,
        old_value: String,
        new_value: String,
    },
}

/// Structured diff between two profiles.
#[derive(Debug, Clone)]
pub struct ProfileDiff {
    pub changes: Vec<DiffChange>,
}

impl ProfileDiff {
    /// Compute a diff between `base` and `other`.
    pub fn compute(base: &Profile, other: &Profile) -> Self {
        let mut changes = Vec::new();

        // --- schema ---
        if base.schema != other.schema {
            changes.push(DiffChange::Modified {
                section: "schema".into(),
                detail: "schema version".into(),
                old_value: base.schema.clone(),
                new_value: other.schema.clone(),
            });
        }

        // --- sim ---
        if base.sim != other.sim {
            match (&base.sim, &other.sim) {
                (None, Some(v)) => changes.push(DiffChange::Added {
                    section: "sim".into(),
                    detail: v.clone(),
                }),
                (Some(v), None) => changes.push(DiffChange::Removed {
                    section: "sim".into(),
                    detail: v.clone(),
                }),
                (Some(a), Some(b)) => changes.push(DiffChange::Modified {
                    section: "sim".into(),
                    detail: "simulator target".into(),
                    old_value: a.clone(),
                    new_value: b.clone(),
                }),
                _ => {}
            }
        }

        // --- aircraft ---
        if base.aircraft != other.aircraft {
            match (&base.aircraft, &other.aircraft) {
                (None, Some(a)) => changes.push(DiffChange::Added {
                    section: "aircraft".into(),
                    detail: a.icao.clone(),
                }),
                (Some(a), None) => changes.push(DiffChange::Removed {
                    section: "aircraft".into(),
                    detail: a.icao.clone(),
                }),
                (Some(a), Some(b)) => changes.push(DiffChange::Modified {
                    section: "aircraft".into(),
                    detail: "aircraft ICAO".into(),
                    old_value: a.icao.clone(),
                    new_value: b.icao.clone(),
                }),
                _ => {}
            }
        }

        // --- axes ---
        // Collect all axis names from both profiles.
        let mut all_axes: Vec<&String> = base.axes.keys().chain(other.axes.keys()).collect();
        all_axes.sort();
        all_axes.dedup();

        for axis_name in all_axes {
            match (base.axes.get(axis_name), other.axes.get(axis_name)) {
                (None, Some(_)) => {
                    changes.push(DiffChange::Added {
                        section: "axes".into(),
                        detail: axis_name.clone(),
                    });
                }
                (Some(_), None) => {
                    changes.push(DiffChange::Removed {
                        section: "axes".into(),
                        detail: axis_name.clone(),
                    });
                }
                (Some(a), Some(b)) if a != b => {
                    Self::diff_axis_config(axis_name, a, b, &mut changes);
                }
                _ => {}
            }
        }

        Self { changes }
    }

    fn diff_axis_config(
        axis_name: &str,
        a: &AxisConfig,
        b: &AxisConfig,
        changes: &mut Vec<DiffChange>,
    ) {
        if a.deadzone != b.deadzone {
            changes.push(DiffChange::Modified {
                section: format!("axes.{axis_name}"),
                detail: "deadzone".into(),
                old_value: format!("{:?}", a.deadzone),
                new_value: format!("{:?}", b.deadzone),
            });
        }
        if a.expo != b.expo {
            changes.push(DiffChange::Modified {
                section: format!("axes.{axis_name}"),
                detail: "expo".into(),
                old_value: format!("{:?}", a.expo),
                new_value: format!("{:?}", b.expo),
            });
        }
        if a.slew_rate != b.slew_rate {
            changes.push(DiffChange::Modified {
                section: format!("axes.{axis_name}"),
                detail: "slew_rate".into(),
                old_value: format!("{:?}", a.slew_rate),
                new_value: format!("{:?}", b.slew_rate),
            });
        }
        if a.curve != b.curve {
            changes.push(DiffChange::Modified {
                section: format!("axes.{axis_name}"),
                detail: "curve".into(),
                old_value: format!("{:?}", a.curve),
                new_value: format!("{:?}", b.curve),
            });
        }
        if a.detents != b.detents {
            changes.push(DiffChange::Modified {
                section: format!("axes.{axis_name}"),
                detail: "detents".into(),
                old_value: format!("{:?}", a.detents),
                new_value: format!("{:?}", b.detents),
            });
        }
        if a.filter != b.filter {
            changes.push(DiffChange::Modified {
                section: format!("axes.{axis_name}"),
                detail: "filter".into(),
                old_value: format!("{:?}", a.filter),
                new_value: format!("{:?}", b.filter),
            });
        }
    }

    /// Human-readable summary of the diff.
    pub fn summary(&self) -> String {
        if self.changes.is_empty() {
            return "No differences.".into();
        }

        let added = self
            .changes
            .iter()
            .filter(|c| matches!(c, DiffChange::Added { .. }))
            .count();
        let removed = self
            .changes
            .iter()
            .filter(|c| matches!(c, DiffChange::Removed { .. }))
            .count();
        let modified = self
            .changes
            .iter()
            .filter(|c| matches!(c, DiffChange::Modified { .. }))
            .count();

        let mut parts = Vec::new();
        if added > 0 {
            parts.push(format!("{added} added"));
        }
        if removed > 0 {
            parts.push(format!("{removed} removed"));
        }
        if modified > 0 {
            parts.push(format!("{modified} modified"));
        }
        parts.join(", ")
    }

    /// Returns `true` when the two profiles are identical.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

impl fmt::Display for ProfileDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Profile diff — {}", self.summary())?;
        for c in &self.changes {
            match c {
                DiffChange::Added { section, detail } => {
                    writeln!(f, "  + [{section}] {detail}")?;
                }
                DiffChange::Removed { section, detail } => {
                    writeln!(f, "  - [{section}] {detail}")?;
                }
                DiffChange::Modified {
                    section,
                    detail,
                    old_value,
                    new_value,
                } => {
                    writeln!(f, "  ~ [{section}] {detail}: {old_value} → {new_value}")?;
                }
            }
        }
        Ok(())
    }
}

// ── Deep validation helpers ──────────────────────────────────────────────────

/// Result of deep profile validation.
#[derive(Debug, Clone)]
pub struct DeepValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl DeepValidationResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Run deep validation on a profile (cross-axis conflicts, curve smoothness,
/// sim-specific settings, plus the standard validation).
pub fn deep_validate(profile: &Profile) -> DeepValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Standard validation.
    if let Err(e) = profile.validate() {
        errors.push(e.to_string());
    }

    // Cross-axis: detect duplicate device+assignment combos via axis names.
    // (Axes are keyed by name in the HashMap so names are unique, but we check
    //  for conflicting deadzone/curve settings that suggest copy-paste errors.)
    check_cross_axis_conflicts(profile, &mut warnings);

    // Deadzone range (0–50% → 0.0–0.5).
    check_deadzone_range(profile, &mut warnings);

    // Curve smoothness.
    check_curve_smoothness(profile, &mut warnings);

    // Sim-specific.
    check_sim_specific(profile, &mut warnings);

    DeepValidationResult { errors, warnings }
}

fn check_cross_axis_conflicts(profile: &Profile, warnings: &mut Vec<String>) {
    let configs: Vec<(&String, &AxisConfig)> = profile.axes.iter().collect();
    for i in 0..configs.len() {
        for j in (i + 1)..configs.len() {
            let (name_a, cfg_a) = configs[i];
            let (name_b, cfg_b) = configs[j];
            // Flag when two different axes share identical non-trivial settings.
            if cfg_a.deadzone == cfg_b.deadzone
                && cfg_a.expo == cfg_b.expo
                && cfg_a.curve == cfg_b.curve
                && cfg_a.deadzone.is_some()
            {
                warnings.push(format!(
                    "Axes '{name_a}' and '{name_b}' have identical configuration — possible copy-paste error"
                ));
            }
        }
    }
}

fn check_deadzone_range(profile: &Profile, warnings: &mut Vec<String>) {
    for (name, cfg) in &profile.axes {
        if let Some(dz) = cfg.deadzone
            && !(0.0..=0.5).contains(&dz)
        {
            warnings.push(format!(
                "Axis '{name}' deadzone {dz} is outside the recommended 0–50% range"
            ));
        }
    }
}

fn check_curve_smoothness(profile: &Profile, warnings: &mut Vec<String>) {
    for (name, cfg) in &profile.axes {
        if let Some(curve) = &cfg.curve {
            if curve.len() < 2 {
                continue;
            }
            // Check for sharp discontinuities: large output jumps relative
            // to small input deltas.
            for i in 1..curve.len() {
                let dx = (curve[i].input - curve[i - 1].input).abs();
                let dy = (curve[i].output - curve[i - 1].output).abs();
                if dx > 0.0 {
                    let slope = dy / dx;
                    // A slope > 10 between adjacent points is likely a
                    // discontinuity.
                    if slope > 10.0 {
                        warnings.push(format!(
                            "Axis '{name}' curve has a sharp discontinuity between points {} and {} (slope {slope:.1})",
                            i - 1, i
                        ));
                    }
                }
            }
        }
    }
}

const KNOWN_SIMS: &[&str] = &["msfs", "xplane", "dcs"];

fn check_sim_specific(profile: &Profile, warnings: &mut Vec<String>) {
    if let Some(sim) = &profile.sim
        && !KNOWN_SIMS.contains(&sim.as_str())
    {
        warnings.push(format!(
            "Unknown simulator '{sim}' — expected one of: {}",
            KNOWN_SIMS.join(", ")
        ));
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AircraftId, CurvePoint, PROFILE_SCHEMA_VERSION};

    fn base_profile() -> Profile {
        let mut axes = HashMap::new();
        axes.insert(
            "pitch".to_string(),
            AxisConfig {
                deadzone: Some(0.03),
                expo: Some(0.2),
                slew_rate: Some(1.2),
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        Profile {
            schema: PROFILE_SCHEMA_VERSION.to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId {
                icao: "C172".to_string(),
            }),
            axes,
            pof_overrides: None,
        }
    }

    // ── Editor set/remove ────────────────────────────────────────────────

    #[test]
    fn editor_set_axis() {
        let mut ed = ProfileEditor::new(base_profile());
        ed.set_axis(
            "roll",
            AxisConfig {
                deadzone: Some(0.05),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        assert!(ed.profile().axes.contains_key("roll"));
        assert_eq!(ed.profile().axes["roll"].deadzone, Some(0.05));
    }

    #[test]
    fn editor_remove_axis() {
        let mut ed = ProfileEditor::new(base_profile());
        assert!(ed.remove_axis("pitch"));
        assert!(!ed.profile().axes.contains_key("pitch"));
        // removing again returns false
        assert!(!ed.remove_axis("pitch"));
    }

    #[test]
    fn editor_set_deadzone_existing_axis() {
        let mut ed = ProfileEditor::new(base_profile());
        ed.set_deadzone("pitch", 0.1);
        assert_eq!(ed.profile().axes["pitch"].deadzone, Some(0.1));
    }

    #[test]
    fn editor_set_deadzone_creates_axis() {
        let mut ed = ProfileEditor::new(base_profile());
        ed.set_deadzone("yaw", 0.08);
        assert!(ed.profile().axes.contains_key("yaw"));
        assert_eq!(ed.profile().axes["yaw"].deadzone, Some(0.08));
    }

    #[test]
    fn editor_set_curve() {
        let mut ed = ProfileEditor::new(base_profile());
        let pts = vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ];
        ed.set_curve("pitch", "linear", pts.clone());
        assert_eq!(ed.profile().axes["pitch"].curve.as_ref().unwrap(), &pts);
    }

    #[test]
    fn editor_set_sim_specific() {
        let mut ed = ProfileEditor::new(base_profile());
        ed.set_sim_specific("msfs", "simconnect_rate", "30");
        assert_eq!(ed.sim_overrides()["msfs"]["simconnect_rate"], "30");
    }

    // ── Merge ────────────────────────────────────────────────────────────

    #[test]
    fn editor_merge_from_adds_new_axis() {
        let mut ed = ProfileEditor::new(base_profile());
        let mut other = base_profile();
        other.axes.insert(
            "rudder".to_string(),
            AxisConfig {
                deadzone: Some(0.1),
                expo: None,
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        ed.merge_from(&other).unwrap();
        assert!(ed.profile().axes.contains_key("rudder"));
    }

    #[test]
    fn editor_merge_from_overrides_existing() {
        let mut ed = ProfileEditor::new(base_profile());
        let mut other = base_profile();
        other.axes.get_mut("pitch").unwrap().expo = Some(0.9);
        ed.merge_from(&other).unwrap();
        assert_eq!(ed.profile().axes["pitch"].expo, Some(0.9));
    }

    // ── Diff ─────────────────────────────────────────────────────────────

    #[test]
    fn diff_identical_profiles_is_empty() {
        let ed = ProfileEditor::new(base_profile());
        let diff = ed.diff(&base_profile());
        assert!(diff.is_empty());
        assert_eq!(diff.summary(), "No differences.");
    }

    #[test]
    fn diff_detects_added_axis() {
        let ed = ProfileEditor::new(base_profile());
        let mut other = base_profile();
        other
            .axes
            .insert("roll".to_string(), AxisConfig::default_empty());
        let diff = ed.diff(&other);
        assert!(!diff.is_empty());
        assert!(
            diff.changes
                .iter()
                .any(|c| matches!(c, DiffChange::Added { section, detail }
                if section == "axes" && detail == "roll"))
        );
    }

    #[test]
    fn diff_detects_removed_axis() {
        let ed = ProfileEditor::new(base_profile());
        let mut other = base_profile();
        other.axes.remove("pitch");
        let diff = ed.diff(&other);
        assert!(
            diff.changes
                .iter()
                .any(|c| matches!(c, DiffChange::Removed { section, detail }
                if section == "axes" && detail == "pitch"))
        );
    }

    #[test]
    fn diff_detects_modified_deadzone() {
        let ed = ProfileEditor::new(base_profile());
        let mut other = base_profile();
        other.axes.get_mut("pitch").unwrap().deadzone = Some(0.1);
        let diff = ed.diff(&other);
        assert!(
            diff.changes
                .iter()
                .any(|c| matches!(c, DiffChange::Modified { section, detail, .. }
                if section == "axes.pitch" && detail == "deadzone"))
        );
    }

    #[test]
    fn diff_summary_format() {
        let ed = ProfileEditor::new(base_profile());
        let mut other = base_profile();
        other.axes.get_mut("pitch").unwrap().expo = Some(0.9);
        other
            .axes
            .insert("roll".to_string(), AxisConfig::default_empty());
        let diff = ed.diff(&other);
        let s = diff.summary();
        assert!(s.contains("added"));
        assert!(s.contains("modified"));
    }

    #[test]
    fn diff_display_trait() {
        let ed = ProfileEditor::new(base_profile());
        let mut other = base_profile();
        other.axes.get_mut("pitch").unwrap().expo = Some(0.5);
        let diff = ed.diff(&other);
        let text = format!("{diff}");
        assert!(text.contains("Profile diff"));
        assert!(text.contains("expo"));
    }

    // ── Deep validation ──────────────────────────────────────────────────

    #[test]
    fn deep_validate_valid_profile() {
        let p = base_profile();
        let r = deep_validate(&p);
        assert!(r.is_ok(), "errors: {:?}", r.errors);
    }

    #[test]
    fn deep_validate_cross_axis_conflict() {
        let mut p = base_profile();
        // Add roll with identical settings to pitch.
        p.axes.insert("roll".to_string(), p.axes["pitch"].clone());
        let r = deep_validate(&p);
        assert!(
            r.warnings.iter().any(|w| w.contains("identical")),
            "expected cross-axis warning, got: {:?}",
            r.warnings
        );
    }

    #[test]
    fn deep_validate_deadzone_out_of_range() {
        let mut p = base_profile();
        p.axes.get_mut("pitch").unwrap().deadzone = Some(0.6);
        let r = deep_validate(&p);
        assert!(
            r.warnings.iter().any(|w| w.contains("deadzone"))
                || r.errors.iter().any(|e| e.contains("deadzone")),
            "expected deadzone issue, got: errors={:?} warnings={:?}",
            r.errors,
            r.warnings
        );
    }

    #[test]
    fn deep_validate_curve_sharp_discontinuity() {
        let mut p = base_profile();
        p.axes.get_mut("pitch").unwrap().curve = Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.01,
                output: 0.9,
            },
            CurvePoint {
                input: 1.0,
                output: 1.0,
            },
        ]);
        let r = deep_validate(&p);
        assert!(
            r.warnings.iter().any(|w| w.contains("discontinuity")),
            "expected curve discontinuity warning, got: {:?}",
            r.warnings
        );
    }

    #[test]
    fn deep_validate_unknown_sim() {
        let mut p = base_profile();
        p.sim = Some("kerbal".to_string());
        let r = deep_validate(&p);
        assert!(
            r.warnings.iter().any(|w| w.contains("Unknown simulator")),
            "expected sim warning, got: {:?}",
            r.warnings
        );
    }

    #[test]
    fn deep_validate_known_sims_pass() {
        for sim in &["msfs", "xplane", "dcs"] {
            let mut p = base_profile();
            p.sim = Some(sim.to_string());
            let r = deep_validate(&p);
            assert!(
                !r.warnings.iter().any(|w| w.contains("Unknown simulator")),
                "sim '{sim}' should be recognized"
            );
        }
    }

    // ── Editor round-trip ────────────────────────────────────────────────

    #[test]
    fn editor_into_profile_returns_modified() {
        let mut ed = ProfileEditor::empty();
        ed.set_axis(
            "pitch",
            AxisConfig {
                deadzone: Some(0.05),
                expo: Some(0.3),
                slew_rate: None,
                detents: vec![],
                curve: None,
                filter: None,
            },
        );
        let p = ed.into_profile();
        assert!(p.axes.contains_key("pitch"));
        assert_eq!(p.axes["pitch"].deadzone, Some(0.05));
    }
}
