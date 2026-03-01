// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! DCS World aircraft module configuration.

use serde::{Deserialize, Serialize};

/// The kind of cockpit control a [`DcsControl`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlType {
    /// A read-only text or numeric display (e.g. UFC scratchpad).
    Display,
    /// A momentary push-button.
    Button,
    /// A two-position on/off toggle.
    Toggle,
    /// A multi-position rotary or selector switch.
    Selector,
    /// A continuous-range axis (e.g. volume knob, gain dial).
    Axis,
}

/// A single DCS-BIOS–style cockpit control or variable.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DcsControl {
    /// Unique variable name within the module (e.g. `"UFC_SCRATCHPAD"`).
    pub name: String,
    /// Logical panel or subsystem category (e.g. `"UFC"`, `"DDI"`, `"MFD"`).
    pub category: String,
    /// What kind of control this is.
    pub control_type: ControlType,
    /// DCS-BIOS export address (decimal).
    pub address: u16,
    /// Human-readable description of the control.
    pub description: String,
}

/// Aircraft-specific axis configuration for a DCS World module.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DcsModule {
    /// Short aircraft identifier as used by DCS (e.g. `"F/A-18C"`).
    pub aircraft: String,
    /// Number of primary control axes exposed by the module.
    pub axis_count: u8,
    /// Throttle range as `[min, max]` normalised to `0.0` – `1.0`.
    pub throttle_range: [f32; 2],
    /// Maximum stick deflection in degrees (total throw, centre-to-stop).
    pub stick_throw: f32,
    /// Known behavioural quirks that require special-case handling.
    pub quirks: Vec<String>,
    /// Optional module version string (e.g. `"2.8"`).
    #[serde(default)]
    pub version: Option<String>,
    /// Optional human-readable module description.
    #[serde(default)]
    pub description: Option<String>,
    /// DCS-BIOS control / variable definitions for this module.
    #[serde(default)]
    pub controls: Vec<DcsControl>,
}

impl DcsModule {
    /// Return an iterator over all controls belonging to a given `category`.
    pub fn controls_by_category_iter<'a>(
        &'a self,
        category: &'a str,
    ) -> impl Iterator<Item = &'a DcsControl> + 'a {
        self.controls.iter().filter(move |c| c.category == category)
    }

    /// Return all controls belonging to a given `category`.
    ///
    /// This is a convenience wrapper around [`DcsModule::controls_by_category_iter`]
    /// that collects the results into a `Vec`. Callers that only need to iterate
    /// can use the iterator-based variant to avoid this allocation.
    pub fn controls_by_category<'a>(&'a self, category: &'a str) -> Vec<&'a DcsControl> {
        self.controls_by_category_iter(category).collect()
    }

    /// Look up a single control by its unique `name`.
    pub fn find_control(&self, name: &str) -> Option<&DcsControl> {
        self.controls.iter().find(|c| c.name == name)
    }

    /// Returns `true` if any control in this module belongs to `category`.
    pub fn has_category(&self, category: &str) -> bool {
        self.controls.iter().any(|c| c.category == category)
    }

    /// Sorted, deduplicated list of all categories present in this module.
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.controls.iter().map(|c| c.category.as_str()).collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    }
}
