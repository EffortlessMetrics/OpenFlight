// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS-BIOS control definitions and module loading.
//!
//! Each aircraft in DCS-BIOS has a set of named controls, each with a category,
//! description, and one or more input/output interfaces. This module provides
//! the types to represent those definitions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::protocol::DcsBiosAddress;

/// Type of input interface a control accepts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputType {
    /// Set the control to a specific state (0..max_value).
    SetState { max_value: u16 },
    /// Increment/decrement by a fixed step (+3200 / -3200 typical).
    FixedStep,
    /// Momentary action (press a button).
    Action,
    /// Variable step with a range.
    VariableStep { max_value: u16, suggested_step: u16 },
}

/// Type of output interface a control provides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputType {
    /// Integer value at a specific address with mask/shift.
    Integer(DcsBiosAddress),
    /// String value at a specific address with a max length.
    String { address: u16, max_length: u16 },
}

/// A single DCS-BIOS control (switch, indicator, display, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DcsBiosControl {
    /// Unique identifier (e.g., `"MASTER_ARM_SW"`).
    pub identifier: String,
    /// Human-readable description.
    pub description: String,
    /// Category/panel name (e.g., `"Master Arm Panel"`).
    pub category: String,
    /// Input interfaces this control accepts.
    pub inputs: Vec<InputType>,
    /// Output interfaces this control provides.
    pub outputs: Vec<OutputType>,
}

/// An aircraft module containing all its DCS-BIOS controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DcsBiosModule {
    /// Module identifier (e.g., `"FA-18C_hornet"`).
    pub name: String,
    /// Base address for this module's allocations.
    pub base_address: u16,
    /// Aircraft variants this module applies to.
    pub aircraft: Vec<String>,
    /// Controls indexed by identifier.
    pub controls: HashMap<String, DcsBiosControl>,
}

impl DcsBiosModule {
    /// Create a new empty module.
    #[must_use]
    pub fn new(name: &str, base_address: u16, aircraft: &[&str]) -> Self {
        Self {
            name: name.to_owned(),
            base_address,
            aircraft: aircraft.iter().map(|s| (*s).to_owned()).collect(),
            controls: HashMap::new(),
        }
    }

    /// Add a control to this module.
    pub fn add_control(&mut self, control: DcsBiosControl) {
        self.controls.insert(control.identifier.clone(), control);
    }

    /// Look up a control by identifier.
    #[must_use]
    pub fn get_control(&self, identifier: &str) -> Option<&DcsBiosControl> {
        self.controls.get(identifier)
    }

    /// Get all controls in a given category.
    #[must_use]
    pub fn controls_in_category(&self, category: &str) -> Vec<&DcsBiosControl> {
        self.controls
            .values()
            .filter(|c| c.category == category)
            .collect()
    }

    /// List all unique categories in this module.
    #[must_use]
    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self
            .controls
            .values()
            .map(|c| c.category.as_str())
            .collect();
        cats.sort_unstable();
        cats.dedup();
        cats
    }

    /// Total number of controls.
    #[must_use]
    pub fn control_count(&self) -> usize {
        self.controls.len()
    }

    /// Get the first integer output address for a control, if any.
    #[must_use]
    pub fn integer_output(&self, identifier: &str) -> Option<&DcsBiosAddress> {
        self.get_control(identifier).and_then(|c| {
            c.outputs.iter().find_map(|o| match o {
                OutputType::Integer(addr) => Some(addr),
                OutputType::String { .. } => None,
            })
        })
    }

    /// Get the string output (address, max_length) for a control, if any.
    #[must_use]
    pub fn string_output(&self, identifier: &str) -> Option<(u16, u16)> {
        self.get_control(identifier).and_then(|c| {
            c.outputs.iter().find_map(|o| match o {
                OutputType::String {
                    address,
                    max_length,
                } => Some((*address, *max_length)),
                OutputType::Integer(_) => None,
            })
        })
    }
}

/// Helper to create a simple toggle switch control (0/1).
#[must_use]
pub fn toggle_switch(
    identifier: &str,
    category: &str,
    description: &str,
    address: u16,
    mask: u16,
    shift: u8,
) -> DcsBiosControl {
    DcsBiosControl {
        identifier: identifier.to_owned(),
        description: description.to_owned(),
        category: category.to_owned(),
        inputs: vec![InputType::SetState { max_value: 1 }],
        outputs: vec![OutputType::Integer(DcsBiosAddress::new(
            address, mask, shift,
        ))],
    }
}

/// Helper to create a multi-position selector control.
#[must_use]
pub fn selector(
    identifier: &str,
    category: &str,
    description: &str,
    address: u16,
    mask: u16,
    shift: u8,
    max_value: u16,
) -> DcsBiosControl {
    DcsBiosControl {
        identifier: identifier.to_owned(),
        description: description.to_owned(),
        category: category.to_owned(),
        inputs: vec![InputType::SetState { max_value }],
        outputs: vec![OutputType::Integer(DcsBiosAddress::new(
            address, mask, shift,
        ))],
    }
}

/// Helper to create a push-button control.
#[must_use]
pub fn push_button(
    identifier: &str,
    category: &str,
    description: &str,
    address: u16,
    mask: u16,
    shift: u8,
) -> DcsBiosControl {
    DcsBiosControl {
        identifier: identifier.to_owned(),
        description: description.to_owned(),
        category: category.to_owned(),
        inputs: vec![InputType::Action],
        outputs: vec![OutputType::Integer(DcsBiosAddress::new(
            address, mask, shift,
        ))],
    }
}

/// Helper to create an indicator light (output only).
#[must_use]
pub fn indicator_light(
    identifier: &str,
    category: &str,
    description: &str,
    address: u16,
    mask: u16,
    shift: u8,
) -> DcsBiosControl {
    DcsBiosControl {
        identifier: identifier.to_owned(),
        description: description.to_owned(),
        category: category.to_owned(),
        inputs: vec![],
        outputs: vec![OutputType::Integer(DcsBiosAddress::new(
            address, mask, shift,
        ))],
    }
}

/// Helper to create a rotary encoder control with fixed step.
#[must_use]
pub fn rotary_encoder(
    identifier: &str,
    category: &str,
    description: &str,
    address: u16,
    mask: u16,
    shift: u8,
    max_value: u16,
) -> DcsBiosControl {
    DcsBiosControl {
        identifier: identifier.to_owned(),
        description: description.to_owned(),
        category: category.to_owned(),
        inputs: vec![InputType::FixedStep, InputType::SetState { max_value }],
        outputs: vec![OutputType::Integer(DcsBiosAddress::new(
            address, mask, shift,
        ))],
    }
}

/// Helper to create a string display (output only).
#[must_use]
pub fn string_display(
    identifier: &str,
    category: &str,
    description: &str,
    address: u16,
    max_length: u16,
) -> DcsBiosControl {
    DcsBiosControl {
        identifier: identifier.to_owned(),
        description: description.to_owned(),
        category: category.to_owned(),
        inputs: vec![],
        outputs: vec![OutputType::String {
            address,
            max_length,
        }],
    }
}

/// Helper to create a 3-position switch control.
#[must_use]
pub fn three_pos_switch(
    identifier: &str,
    category: &str,
    description: &str,
    address: u16,
    mask: u16,
    shift: u8,
) -> DcsBiosControl {
    DcsBiosControl {
        identifier: identifier.to_owned(),
        description: description.to_owned(),
        category: category.to_owned(),
        inputs: vec![InputType::SetState { max_value: 2 }],
        outputs: vec![OutputType::Integer(DcsBiosAddress::new(
            address, mask, shift,
        ))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_add_and_get_control() {
        let mut module = DcsBiosModule::new("test", 0x0000, &["TestAircraft"]);
        let ctrl = toggle_switch("TEST_SW", "Test Panel", "Test Switch", 0x0000, 0x0001, 0);
        module.add_control(ctrl);

        assert_eq!(module.control_count(), 1);
        assert!(module.get_control("TEST_SW").is_some());
        assert!(module.get_control("NONEXISTENT").is_none());
    }

    #[test]
    fn module_categories() {
        let mut module = DcsBiosModule::new("test", 0x0000, &["TestAircraft"]);
        module.add_control(toggle_switch(
            "SW1", "Panel A", "Switch 1", 0x0000, 0x0001, 0,
        ));
        module.add_control(toggle_switch(
            "SW2", "Panel B", "Switch 2", 0x0002, 0x0001, 0,
        ));
        module.add_control(toggle_switch(
            "SW3", "Panel A", "Switch 3", 0x0004, 0x0001, 0,
        ));

        let cats = module.categories();
        assert_eq!(cats.len(), 2);
        assert!(cats.contains(&"Panel A"));
        assert!(cats.contains(&"Panel B"));
    }

    #[test]
    fn controls_in_category_filter() {
        let mut module = DcsBiosModule::new("test", 0x0000, &["TestAircraft"]);
        module.add_control(toggle_switch(
            "SW1", "Panel A", "Switch 1", 0x0000, 0x0001, 0,
        ));
        module.add_control(toggle_switch(
            "SW2", "Panel B", "Switch 2", 0x0002, 0x0001, 0,
        ));

        assert_eq!(module.controls_in_category("Panel A").len(), 1);
        assert_eq!(module.controls_in_category("Panel C").len(), 0);
    }

    #[test]
    fn integer_output_lookup() {
        let mut module = DcsBiosModule::new("test", 0x0000, &["TestAircraft"]);
        module.add_control(toggle_switch("SW1", "Panel", "Switch", 0x1000, 0x0100, 8));

        let addr = module.integer_output("SW1").unwrap();
        assert_eq!(addr.address, 0x1000);
        assert_eq!(addr.mask, 0x0100);
        assert_eq!(addr.shift, 8);
    }

    #[test]
    fn string_output_lookup() {
        let mut module = DcsBiosModule::new("test", 0x0000, &["TestAircraft"]);
        module.add_control(string_display("DISP1", "Panel", "Display", 0x2000, 8));

        let (addr, len) = module.string_output("DISP1").unwrap();
        assert_eq!(addr, 0x2000);
        assert_eq!(len, 8);
    }

    #[test]
    fn module_serde_roundtrip() {
        let mut module = DcsBiosModule::new("test", 0x7400, &["FA-18C_hornet"]);
        module.add_control(toggle_switch("SW1", "Panel", "Switch", 0x7400, 0x0001, 0));

        let json = serde_json::to_string(&module).unwrap();
        let restored: DcsBiosModule = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "test");
        assert_eq!(restored.control_count(), 1);
    }

    #[test]
    fn helper_push_button_has_action_input() {
        let ctrl = push_button("BTN1", "Panel", "Button", 0x0000, 0x0001, 0);
        assert!(ctrl.inputs.iter().any(|i| matches!(i, InputType::Action)));
    }

    #[test]
    fn helper_rotary_has_fixed_step_and_set_state() {
        let ctrl = rotary_encoder("ROT1", "Panel", "Rotary", 0x0000, 0xFFFF, 0, 65535);
        assert!(
            ctrl.inputs
                .iter()
                .any(|i| matches!(i, InputType::FixedStep))
        );
        assert!(
            ctrl.inputs
                .iter()
                .any(|i| matches!(i, InputType::SetState { .. }))
        );
    }

    #[test]
    fn helper_indicator_has_no_inputs() {
        let ctrl = indicator_light("LT1", "Panel", "Light", 0x0000, 0x0001, 0);
        assert!(ctrl.inputs.is_empty());
    }
}
