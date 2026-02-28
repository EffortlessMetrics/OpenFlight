// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Action templates for common flight simulation functions
//!
//! Provides ready-made [`ActionTemplate`] definitions for autopilot, comms,
//! navigation, lights, and systems controls that can be bound to StreamDeck
//! keys.

use crate::render::IconTheme;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Functional category that an action belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionCategory {
    Autopilot,
    Communication,
    Navigation,
    Lights,
    Systems,
}

impl ActionCategory {
    pub fn icon_theme(&self) -> IconTheme {
        match self {
            Self::Autopilot => IconTheme::Autopilot,
            Self::Communication => IconTheme::Communication,
            Self::Navigation => IconTheme::Navigation,
            Self::Lights => IconTheme::Lights,
            Self::Systems => IconTheme::Systems,
        }
    }

    /// All categories.
    pub fn all() -> &'static [ActionCategory] {
        &[
            Self::Autopilot,
            Self::Communication,
            Self::Navigation,
            Self::Lights,
            Self::Systems,
        ]
    }
}

/// How a button behaves when pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionBehavior {
    /// Press toggles between two states (on/off).
    Toggle,
    /// Press fires once (momentary).
    Momentary,
    /// Press increments / long-press decrements a value.
    Encoder,
}

/// A reusable action template that can be placed on any StreamDeck key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTemplate {
    /// Unique identifier, e.g. `"com.flighthub.ap-toggle"`.
    pub id: String,
    /// Human-readable name shown in the configurator.
    pub name: String,
    /// Short label rendered on the key face.
    pub key_label: String,
    pub category: ActionCategory,
    pub behavior: ActionBehavior,
    /// SimConnect / X-Plane event fired on press.
    pub command_on: String,
    /// Event fired on release / second press (toggle off). Empty for momentary.
    pub command_off: String,
    /// Telemetry variable used to reflect current state on the key icon.
    pub feedback_variable: String,
    /// Tooltip shown in the property inspector.
    pub tooltip: String,
}

impl ActionTemplate {
    /// Convenience builder.
    fn toggle(
        id: &str,
        name: &str,
        label: &str,
        cat: ActionCategory,
        cmd: &str,
        var: &str,
    ) -> Self {
        Self {
            id: format!("com.flighthub.{id}"),
            name: name.to_string(),
            key_label: label.to_string(),
            category: cat,
            behavior: ActionBehavior::Toggle,
            command_on: cmd.to_string(),
            command_off: cmd.to_string(),
            feedback_variable: var.to_string(),
            tooltip: format!("Toggle {name}"),
        }
    }

    fn momentary(id: &str, name: &str, label: &str, cat: ActionCategory, cmd: &str) -> Self {
        Self {
            id: format!("com.flighthub.{id}"),
            name: name.to_string(),
            key_label: label.to_string(),
            category: cat,
            behavior: ActionBehavior::Momentary,
            command_on: cmd.to_string(),
            command_off: String::new(),
            feedback_variable: String::new(),
            tooltip: format!("Press {name}"),
        }
    }
}

// ── Built-in template library ────────────────────────────────────────────────

/// Returns all built-in action templates.
pub fn builtin_templates() -> Vec<ActionTemplate> {
    let mut t = Vec::new();
    t.extend(autopilot_templates());
    t.extend(communication_templates());
    t.extend(navigation_templates());
    t.extend(lights_templates());
    t.extend(systems_templates());
    t
}

/// Autopilot action templates.
pub fn autopilot_templates() -> Vec<ActionTemplate> {
    vec![
        ActionTemplate::toggle(
            "ap-toggle",
            "AP Master",
            "AP",
            ActionCategory::Autopilot,
            "AP_MASTER",
            "AUTOPILOT_MASTER",
        ),
        ActionTemplate::toggle(
            "ap-hdg",
            "Heading Hold",
            "HDG",
            ActionCategory::Autopilot,
            "AP_HDG_HOLD",
            "AUTOPILOT_HEADING_LOCK",
        ),
        ActionTemplate::toggle(
            "ap-alt",
            "Altitude Hold",
            "ALT",
            ActionCategory::Autopilot,
            "AP_ALT_HOLD",
            "AUTOPILOT_ALTITUDE_LOCK",
        ),
        ActionTemplate::toggle(
            "ap-vs",
            "Vertical Speed",
            "VS",
            ActionCategory::Autopilot,
            "AP_VS_HOLD",
            "AUTOPILOT_VERTICAL_HOLD",
        ),
        ActionTemplate::toggle(
            "ap-apr",
            "Approach Mode",
            "APR",
            ActionCategory::Autopilot,
            "AP_APR_HOLD",
            "AUTOPILOT_APPROACH_HOLD",
        ),
        ActionTemplate::toggle(
            "ap-nav",
            "NAV Mode",
            "NAV",
            ActionCategory::Autopilot,
            "AP_NAV1_HOLD",
            "AUTOPILOT_NAV1_LOCK",
        ),
    ]
}

/// Communication action templates.
pub fn communication_templates() -> Vec<ActionTemplate> {
    vec![
        ActionTemplate::momentary(
            "com1-swap",
            "COM1 Standby Swap",
            "COM1\nSWAP",
            ActionCategory::Communication,
            "COM_STBY_RADIO_SWAP",
        ),
        ActionTemplate::momentary(
            "com2-swap",
            "COM2 Standby Swap",
            "COM2\nSWAP",
            ActionCategory::Communication,
            "COM2_RADIO_SWAP",
        ),
    ]
}

/// Navigation action templates.
pub fn navigation_templates() -> Vec<ActionTemplate> {
    vec![
        ActionTemplate::momentary(
            "nav1-swap",
            "NAV1 Standby Swap",
            "NAV1\nSWAP",
            ActionCategory::Navigation,
            "NAV1_RADIO_SWAP",
        ),
        ActionTemplate::momentary(
            "nav2-swap",
            "NAV2 Standby Swap",
            "NAV2\nSWAP",
            ActionCategory::Navigation,
            "NAV2_RADIO_SWAP",
        ),
        ActionTemplate {
            id: "com.flighthub.vor-obs-inc".to_string(),
            name: "VOR OBS Increment".to_string(),
            key_label: "OBS\n+".to_string(),
            category: ActionCategory::Navigation,
            behavior: ActionBehavior::Encoder,
            command_on: "VOR1_OBI_INC".to_string(),
            command_off: "VOR1_OBI_DEC".to_string(),
            feedback_variable: "NAV_OBS:1".to_string(),
            tooltip: "Increment / decrement VOR1 OBS".to_string(),
        },
        ActionTemplate::toggle(
            "ils-loc",
            "ILS Localizer",
            "LOC",
            ActionCategory::Navigation,
            "AP_LOC_HOLD",
            "AUTOPILOT_APPROACH_HOLD",
        ),
    ]
}

/// Lights action templates.
pub fn lights_templates() -> Vec<ActionTemplate> {
    vec![
        ActionTemplate::toggle(
            "light-landing",
            "Landing Lights",
            "LAND",
            ActionCategory::Lights,
            "LANDING_LIGHTS_TOGGLE",
            "LIGHT_LANDING",
        ),
        ActionTemplate::toggle(
            "light-taxi",
            "Taxi Lights",
            "TAXI",
            ActionCategory::Lights,
            "TOGGLE_TAXI_LIGHTS",
            "LIGHT_TAXI",
        ),
        ActionTemplate::toggle(
            "light-beacon",
            "Beacon Light",
            "BCN",
            ActionCategory::Lights,
            "TOGGLE_BEACON_LIGHTS",
            "LIGHT_BEACON",
        ),
        ActionTemplate::toggle(
            "light-nav",
            "Navigation Lights",
            "NAV",
            ActionCategory::Lights,
            "TOGGLE_NAV_LIGHTS",
            "LIGHT_NAV",
        ),
        ActionTemplate::toggle(
            "light-strobe",
            "Strobe Lights",
            "STRB",
            ActionCategory::Lights,
            "STROBES_TOGGLE",
            "LIGHT_STROBE",
        ),
    ]
}

/// Systems action templates.
pub fn systems_templates() -> Vec<ActionTemplate> {
    vec![
        ActionTemplate::toggle(
            "sys-fuel-pump",
            "Fuel Pump",
            "FUEL\nPUMP",
            ActionCategory::Systems,
            "TOGGLE_ELECT_FUEL_PUMP1",
            "GENERAL_ENG_FUEL_PUMP_SWITCH:1",
        ),
        ActionTemplate::toggle(
            "sys-battery",
            "Battery Master",
            "BAT",
            ActionCategory::Systems,
            "TOGGLE_MASTER_BATTERY",
            "ELECTRICAL_MASTER_BATTERY",
        ),
        ActionTemplate::toggle(
            "sys-alternator",
            "Alternator",
            "ALT\nGEN",
            ActionCategory::Systems,
            "TOGGLE_MASTER_ALTERNATOR",
            "GENERAL_ENG_MASTER_ALTERNATOR:1",
        ),
        ActionTemplate::toggle(
            "sys-pitot-heat",
            "Pitot Heat",
            "PITOT\nHEAT",
            ActionCategory::Systems,
            "PITOT_HEAT_TOGGLE",
            "PITOT_HEAT",
        ),
    ]
}

/// Registry providing fast lookup of templates by id or category.
pub struct ActionRegistry {
    templates: HashMap<String, ActionTemplate>,
}

impl ActionRegistry {
    /// Build a registry from all built-in templates.
    pub fn builtin() -> Self {
        let templates = builtin_templates()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();
        Self { templates }
    }

    /// Add a custom template.
    pub fn register(&mut self, template: ActionTemplate) {
        self.templates.insert(template.id.clone(), template);
    }

    /// Lookup by id.
    pub fn get(&self, id: &str) -> Option<&ActionTemplate> {
        self.templates.get(id)
    }

    /// All templates in a given category.
    pub fn by_category(&self, category: ActionCategory) -> Vec<&ActionTemplate> {
        self.templates
            .values()
            .filter(|t| t.category == category)
            .collect()
    }

    /// Total number of registered templates.
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.templates.is_empty()
    }

    /// All registered template ids.
    pub fn ids(&self) -> Vec<&str> {
        self.templates.keys().map(String::as_str).collect()
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Template counts ────────────────────────────────────────────────

    #[test]
    fn test_autopilot_templates_count() {
        let t = autopilot_templates();
        assert_eq!(t.len(), 6, "AP, HDG, ALT, VS, APR, NAV");
    }

    #[test]
    fn test_communication_templates_count() {
        let t = communication_templates();
        assert_eq!(t.len(), 2, "COM1 swap, COM2 swap");
    }

    #[test]
    fn test_navigation_templates_count() {
        let t = navigation_templates();
        assert_eq!(t.len(), 4, "NAV1, NAV2, VOR OBS, ILS LOC");
    }

    #[test]
    fn test_lights_templates_count() {
        let t = lights_templates();
        assert_eq!(t.len(), 5, "landing, taxi, beacon, nav, strobe");
    }

    #[test]
    fn test_systems_templates_count() {
        let t = systems_templates();
        assert_eq!(t.len(), 4, "fuel pump, battery, alternator, pitot heat");
    }

    #[test]
    fn test_builtin_total() {
        let all = builtin_templates();
        assert_eq!(all.len(), 6 + 2 + 4 + 5 + 4);
    }

    // ── Template properties ────────────────────────────────────────────

    #[test]
    fn test_ids_are_namespaced() {
        for t in builtin_templates() {
            assert!(
                t.id.starts_with("com.flighthub."),
                "id {} must start with com.flighthub.",
                t.id
            );
        }
    }

    #[test]
    fn test_ids_are_unique() {
        let all = builtin_templates();
        let mut ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "duplicate action ids found");
    }

    #[test]
    fn test_toggle_has_feedback_variable() {
        for t in builtin_templates() {
            if t.behavior == ActionBehavior::Toggle {
                assert!(
                    !t.feedback_variable.is_empty(),
                    "{} is toggle but has no feedback var",
                    t.id
                );
            }
        }
    }

    #[test]
    fn test_momentary_has_empty_command_off() {
        for t in builtin_templates() {
            if t.behavior == ActionBehavior::Momentary {
                assert!(
                    t.command_off.is_empty(),
                    "{} is momentary but has command_off",
                    t.id
                );
            }
        }
    }

    // ── Category helpers ───────────────────────────────────────────────

    #[test]
    fn test_all_categories() {
        assert_eq!(ActionCategory::all().len(), 5);
    }

    #[test]
    fn test_category_icon_themes() {
        assert_eq!(ActionCategory::Autopilot.icon_theme(), IconTheme::Autopilot);
        assert_eq!(ActionCategory::Lights.icon_theme(), IconTheme::Lights);
    }

    // ── ActionRegistry ─────────────────────────────────────────────────

    #[test]
    fn test_registry_builtin() {
        let reg = ActionRegistry::builtin();
        assert_eq!(reg.len(), builtin_templates().len());
        assert!(!reg.is_empty());
    }

    #[test]
    fn test_registry_lookup() {
        let reg = ActionRegistry::builtin();
        let ap = reg.get("com.flighthub.ap-toggle").unwrap();
        assert_eq!(ap.name, "AP Master");
    }

    #[test]
    fn test_registry_by_category() {
        let reg = ActionRegistry::builtin();
        let lights = reg.by_category(ActionCategory::Lights);
        assert_eq!(lights.len(), 5);
    }

    #[test]
    fn test_registry_custom_template() {
        let mut reg = ActionRegistry::builtin();
        let before = reg.len();
        reg.register(ActionTemplate::momentary(
            "custom-action",
            "Custom",
            "CUS",
            ActionCategory::Systems,
            "CUSTOM_CMD",
        ));
        assert_eq!(reg.len(), before + 1);
        assert!(reg.get("com.flighthub.custom-action").is_some());
    }

    #[test]
    fn test_registry_ids() {
        let reg = ActionRegistry::builtin();
        let ids = reg.ids();
        assert!(ids.contains(&"com.flighthub.ap-toggle"));
        assert!(ids.contains(&"com.flighthub.light-landing"));
    }
}
