// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Button-to-SimConnect event mapping
//!
//! Maps physical device buttons to SimConnect client events. Provides a
//! catalog of known events, a runtime mapping table, and export helpers
//! for profile persistence.

use std::collections::HashMap;

/// Category of a SimConnect client event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SimEventCategory {
    FlightControls,
    Engine,
    Autopilot,
    Electrical,
    Radios,
    Views,
    Misc,
}

/// Definition of a SimConnect client event.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SimEventDef {
    /// SimConnect event name (e.g. `"GEAR_TOGGLE"`).
    pub name: &'static str,
    /// Functional category.
    pub category: SimEventCategory,
    /// Human-readable description.
    pub description: &'static str,
    /// Whether this event toggles state (press-on / press-off).
    pub toggle: bool,
}

/// Static catalog of well-known SimConnect client events.
pub const SIM_EVENT_CATALOG: &[SimEventDef] = &[
    // ── Flight Controls ──────────────────────────────────────────
    SimEventDef {
        name: "AXIS_ELEVATOR_SET",
        category: SimEventCategory::FlightControls,
        description: "Set elevator axis position",
        toggle: false,
    },
    SimEventDef {
        name: "AXIS_AILERONS_SET",
        category: SimEventCategory::FlightControls,
        description: "Set ailerons axis position",
        toggle: false,
    },
    SimEventDef {
        name: "AXIS_RUDDER_SET",
        category: SimEventCategory::FlightControls,
        description: "Set rudder axis position",
        toggle: false,
    },
    SimEventDef {
        name: "AXIS_THROTTLE_SET",
        category: SimEventCategory::FlightControls,
        description: "Set throttle axis position",
        toggle: false,
    },
    SimEventDef {
        name: "AXIS_MIXTURE_SET",
        category: SimEventCategory::FlightControls,
        description: "Set mixture lever position",
        toggle: false,
    },
    SimEventDef {
        name: "AXIS_PROPELLER_SET",
        category: SimEventCategory::FlightControls,
        description: "Set propeller lever position",
        toggle: false,
    },
    SimEventDef {
        name: "AXIS_SPOILER_SET",
        category: SimEventCategory::FlightControls,
        description: "Set spoiler lever position",
        toggle: false,
    },
    SimEventDef {
        name: "FLAPS_INCR",
        category: SimEventCategory::FlightControls,
        description: "Increment flaps one notch",
        toggle: false,
    },
    SimEventDef {
        name: "FLAPS_DECR",
        category: SimEventCategory::FlightControls,
        description: "Decrement flaps one notch",
        toggle: false,
    },
    SimEventDef {
        name: "FLAPS_SET",
        category: SimEventCategory::FlightControls,
        description: "Set flap handle to specific position",
        toggle: false,
    },
    SimEventDef {
        name: "FLAPS_UP",
        category: SimEventCategory::FlightControls,
        description: "Retract flaps fully",
        toggle: false,
    },
    SimEventDef {
        name: "FLAPS_DOWN",
        category: SimEventCategory::FlightControls,
        description: "Extend flaps fully",
        toggle: false,
    },
    SimEventDef {
        name: "GEAR_TOGGLE",
        category: SimEventCategory::FlightControls,
        description: "Toggle landing gear",
        toggle: true,
    },
    SimEventDef {
        name: "GEAR_UP",
        category: SimEventCategory::FlightControls,
        description: "Retract landing gear",
        toggle: false,
    },
    SimEventDef {
        name: "GEAR_DOWN",
        category: SimEventCategory::FlightControls,
        description: "Extend landing gear",
        toggle: false,
    },
    SimEventDef {
        name: "SPOILERS_ARM_TOGGLE",
        category: SimEventCategory::FlightControls,
        description: "Toggle spoiler arming",
        toggle: true,
    },
    SimEventDef {
        name: "PARKING_BRAKES",
        category: SimEventCategory::FlightControls,
        description: "Toggle parking brakes",
        toggle: true,
    },
    SimEventDef {
        name: "ELEV_TRIM_UP",
        category: SimEventCategory::FlightControls,
        description: "Trim elevator nose-up",
        toggle: false,
    },
    SimEventDef {
        name: "ELEV_TRIM_DN",
        category: SimEventCategory::FlightControls,
        description: "Trim elevator nose-down",
        toggle: false,
    },
    // ── Engine ───────────────────────────────────────────────────
    SimEventDef {
        name: "THROTTLE_FULL",
        category: SimEventCategory::Engine,
        description: "Set throttle to full",
        toggle: false,
    },
    SimEventDef {
        name: "THROTTLE_CUT",
        category: SimEventCategory::Engine,
        description: "Set throttle to idle/cutoff",
        toggle: false,
    },
    SimEventDef {
        name: "THROTTLE_INCR",
        category: SimEventCategory::Engine,
        description: "Increase throttle slightly",
        toggle: false,
    },
    SimEventDef {
        name: "THROTTLE_DECR",
        category: SimEventCategory::Engine,
        description: "Decrease throttle slightly",
        toggle: false,
    },
    SimEventDef {
        name: "MIXTURE_RICH",
        category: SimEventCategory::Engine,
        description: "Mixture full rich",
        toggle: false,
    },
    SimEventDef {
        name: "MIXTURE_LEAN",
        category: SimEventCategory::Engine,
        description: "Mixture full lean",
        toggle: false,
    },
    SimEventDef {
        name: "ENGINE_AUTO_START",
        category: SimEventCategory::Engine,
        description: "Auto-start engine(s)",
        toggle: false,
    },
    SimEventDef {
        name: "ENGINE_AUTO_SHUTDOWN",
        category: SimEventCategory::Engine,
        description: "Auto-shutdown engine(s)",
        toggle: false,
    },
    SimEventDef {
        name: "TOGGLE_STARTER1",
        category: SimEventCategory::Engine,
        description: "Toggle engine 1 starter",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_STARTER2",
        category: SimEventCategory::Engine,
        description: "Toggle engine 2 starter",
        toggle: true,
    },
    // ── Autopilot ────────────────────────────────────────────────
    SimEventDef {
        name: "AP_MASTER",
        category: SimEventCategory::Autopilot,
        description: "Toggle autopilot master",
        toggle: true,
    },
    SimEventDef {
        name: "AP_ALT_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle altitude hold",
        toggle: true,
    },
    SimEventDef {
        name: "AP_HDG_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle heading hold",
        toggle: true,
    },
    SimEventDef {
        name: "AP_SPD_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle speed hold",
        toggle: true,
    },
    SimEventDef {
        name: "AP_VS_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle vertical speed hold",
        toggle: true,
    },
    SimEventDef {
        name: "AP_NAV1_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle NAV1 hold (LNAV)",
        toggle: true,
    },
    SimEventDef {
        name: "AP_APR_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle approach hold",
        toggle: true,
    },
    SimEventDef {
        name: "AP_LOC_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle localizer hold",
        toggle: true,
    },
    SimEventDef {
        name: "AP_BC_HOLD",
        category: SimEventCategory::Autopilot,
        description: "Toggle back-course hold",
        toggle: true,
    },
    SimEventDef {
        name: "FLIGHT_LEVEL_CHANGE",
        category: SimEventCategory::Autopilot,
        description: "Toggle flight level change mode",
        toggle: true,
    },
    // ── Electrical ───────────────────────────────────────────────
    SimEventDef {
        name: "TOGGLE_MASTER_BATTERY",
        category: SimEventCategory::Electrical,
        description: "Toggle master battery",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_MASTER_ALTERNATOR",
        category: SimEventCategory::Electrical,
        description: "Toggle master alternator",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_AVIONICS_MASTER",
        category: SimEventCategory::Electrical,
        description: "Toggle avionics master switch",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_NAV_LIGHTS",
        category: SimEventCategory::Electrical,
        description: "Toggle navigation lights",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_BEACON_LIGHTS",
        category: SimEventCategory::Electrical,
        description: "Toggle beacon lights",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_STROBE_LIGHTS",
        category: SimEventCategory::Electrical,
        description: "Toggle strobe lights",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_LANDING_LIGHTS",
        category: SimEventCategory::Electrical,
        description: "Toggle landing lights",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_TAXI_LIGHTS",
        category: SimEventCategory::Electrical,
        description: "Toggle taxi lights",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_LOGO_LIGHTS",
        category: SimEventCategory::Electrical,
        description: "Toggle logo lights",
        toggle: true,
    },
    SimEventDef {
        name: "TOGGLE_WING_LIGHTS",
        category: SimEventCategory::Electrical,
        description: "Toggle wing lights",
        toggle: true,
    },
    SimEventDef {
        name: "PITOT_HEAT_TOGGLE",
        category: SimEventCategory::Electrical,
        description: "Toggle pitot heat",
        toggle: true,
    },
    // ── Radios ───────────────────────────────────────────────────
    SimEventDef {
        name: "COM1_TRANSMIT_SELECT",
        category: SimEventCategory::Radios,
        description: "Select COM1 for transmit",
        toggle: false,
    },
    SimEventDef {
        name: "COM2_TRANSMIT_SELECT",
        category: SimEventCategory::Radios,
        description: "Select COM2 for transmit",
        toggle: false,
    },
    SimEventDef {
        name: "COM_STBY_RADIO_SWAP",
        category: SimEventCategory::Radios,
        description: "Swap COM1 active/standby",
        toggle: false,
    },
    SimEventDef {
        name: "NAV1_RADIO_SWAP",
        category: SimEventCategory::Radios,
        description: "Swap NAV1 active/standby",
        toggle: false,
    },
    SimEventDef {
        name: "ADF_COMPLETE_SET",
        category: SimEventCategory::Radios,
        description: "Set ADF frequency",
        toggle: false,
    },
    SimEventDef {
        name: "XPNDR_SET",
        category: SimEventCategory::Radios,
        description: "Set transponder code",
        toggle: false,
    },
    // ── Views ────────────────────────────────────────────────────
    SimEventDef {
        name: "VIEW_COCKPIT_FORWARD",
        category: SimEventCategory::Views,
        description: "Cockpit forward view",
        toggle: false,
    },
    SimEventDef {
        name: "VIEW_VIRTUAL_COCKPIT",
        category: SimEventCategory::Views,
        description: "Virtual cockpit view",
        toggle: false,
    },
    SimEventDef {
        name: "VIEW_CHASE",
        category: SimEventCategory::Views,
        description: "Chase camera view",
        toggle: false,
    },
    SimEventDef {
        name: "VIEW_EXTERNAL",
        category: SimEventCategory::Views,
        description: "External view",
        toggle: false,
    },
    // ── Miscellaneous ────────────────────────────────────────────
    SimEventDef {
        name: "PAUSE_TOGGLE",
        category: SimEventCategory::Misc,
        description: "Toggle pause",
        toggle: true,
    },
    SimEventDef {
        name: "SIM_RATE_INCR",
        category: SimEventCategory::Misc,
        description: "Increase simulation rate",
        toggle: false,
    },
    SimEventDef {
        name: "SIM_RATE_DECR",
        category: SimEventCategory::Misc,
        description: "Decrease simulation rate",
        toggle: false,
    },
];

/// Runtime mapper that binds device buttons to one or more SimConnect events.
pub struct SimEventMapper {
    /// button-key → list of event names
    mappings: HashMap<String, Vec<&'static str>>,
}

impl SimEventMapper {
    /// Create an empty mapper.
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Bind a device button to a SimConnect event (appends if already mapped).
    pub fn map_button(&mut self, device_button: &str, event_name: &'static str) {
        self.mappings
            .entry(device_button.to_string())
            .or_default()
            .push(event_name);
    }

    /// Return the events bound to a button, if any.
    pub fn get_events(&self, device_button: &str) -> Option<&[&'static str]> {
        self.mappings.get(device_button).map(|v| v.as_slice())
    }

    /// List buttons that have no mapping yet from the provided full set.
    pub fn unmapped_buttons<'a>(&self, all_buttons: &[&'a str]) -> Vec<&'a str> {
        all_buttons
            .iter()
            .filter(|b| !self.mappings.contains_key(**b))
            .copied()
            .collect()
    }

    /// Export the mapping table as a list of `(button, event_name)` pairs.
    pub fn export_mapping(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (btn, events) in &self.mappings {
            for ev in events {
                out.push((btn.clone(), (*ev).to_string()));
            }
        }
        out.sort();
        out
    }

    /// Remove all mappings for a given button.
    pub fn unmap_button(&mut self, device_button: &str) {
        self.mappings.remove(device_button);
    }

    /// Total number of mapped buttons.
    pub fn mapped_button_count(&self) -> usize {
        self.mappings.len()
    }
}

impl Default for SimEventMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// Look up a [`SimEventDef`] in the static catalog by name.
pub fn catalog_lookup(name: &str) -> Option<&'static SimEventDef> {
    SIM_EVENT_CATALOG.iter().find(|e| e.name == name)
}

/// Return all catalog events belonging to the given category.
pub fn catalog_by_category(cat: SimEventCategory) -> Vec<&'static SimEventDef> {
    SIM_EVENT_CATALOG
        .iter()
        .filter(|e| e.category == cat)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_at_least_50_events() {
        assert!(
            SIM_EVENT_CATALOG.len() >= 50,
            "catalog must have ≥50 events, got {}",
            SIM_EVENT_CATALOG.len()
        );
    }

    #[test]
    fn catalog_lookup_known() {
        let ev = catalog_lookup("GEAR_TOGGLE").expect("GEAR_TOGGLE must exist");
        assert_eq!(ev.category, SimEventCategory::FlightControls);
        assert!(ev.toggle);
    }

    #[test]
    fn catalog_lookup_missing() {
        assert!(catalog_lookup("DOES_NOT_EXIST").is_none());
    }

    #[test]
    fn catalog_by_category_autopilot() {
        let ap = catalog_by_category(SimEventCategory::Autopilot);
        assert!(ap.len() >= 5);
        for e in &ap {
            assert_eq!(e.category, SimEventCategory::Autopilot);
        }
    }

    #[test]
    fn mapper_map_and_get() {
        let mut mapper = SimEventMapper::new();
        mapper.map_button("btn_1", "GEAR_TOGGLE");
        mapper.map_button("btn_1", "TOGGLE_NAV_LIGHTS");
        mapper.map_button("btn_2", "AP_MASTER");

        let events = mapper.get_events("btn_1").expect("btn_1 mapped");
        assert_eq!(events.len(), 2);
        assert!(events.contains(&"GEAR_TOGGLE"));
        assert!(events.contains(&"TOGGLE_NAV_LIGHTS"));

        let events2 = mapper.get_events("btn_2").expect("btn_2 mapped");
        assert_eq!(events2, &["AP_MASTER"]);

        assert!(mapper.get_events("btn_99").is_none());
    }

    #[test]
    fn mapper_unmapped_buttons() {
        let mut mapper = SimEventMapper::new();
        mapper.map_button("btn_1", "GEAR_TOGGLE");

        let all = &["btn_1", "btn_2", "btn_3"];
        let unmapped = mapper.unmapped_buttons(all);
        assert_eq!(unmapped.len(), 2);
        assert!(unmapped.contains(&"btn_2"));
        assert!(unmapped.contains(&"btn_3"));
        assert!(!unmapped.contains(&"btn_1"));
    }

    #[test]
    fn mapper_export() {
        let mut mapper = SimEventMapper::new();
        mapper.map_button("btn_2", "AP_MASTER");
        mapper.map_button("btn_1", "GEAR_TOGGLE");

        let exported = mapper.export_mapping();
        assert_eq!(exported.len(), 2);
        // Sorted by button name
        assert_eq!(exported[0].0, "btn_1");
        assert_eq!(exported[1].0, "btn_2");
    }

    #[test]
    fn mapper_unmap() {
        let mut mapper = SimEventMapper::new();
        mapper.map_button("btn_1", "GEAR_TOGGLE");
        assert!(mapper.get_events("btn_1").is_some());
        mapper.unmap_button("btn_1");
        assert!(mapper.get_events("btn_1").is_none());
    }

    #[test]
    fn mapper_count() {
        let mut mapper = SimEventMapper::new();
        assert_eq!(mapper.mapped_button_count(), 0);
        mapper.map_button("btn_1", "GEAR_TOGGLE");
        mapper.map_button("btn_2", "AP_MASTER");
        assert_eq!(mapper.mapped_button_count(), 2);
    }

    #[test]
    fn every_category_in_catalog() {
        let categories = [
            SimEventCategory::FlightControls,
            SimEventCategory::Engine,
            SimEventCategory::Autopilot,
            SimEventCategory::Electrical,
            SimEventCategory::Radios,
            SimEventCategory::Views,
            SimEventCategory::Misc,
        ];
        for cat in categories {
            assert!(
                !catalog_by_category(cat).is_empty(),
                "category {cat:?} must have at least one event"
            );
        }
    }

    #[test]
    fn toggle_events_are_correct() {
        // AP events should all be toggles
        for ev in catalog_by_category(SimEventCategory::Autopilot) {
            assert!(ev.toggle, "{} should be a toggle event", ev.name);
        }
        // Axis events should not be toggles
        let axis = catalog_lookup("AXIS_ELEVATOR_SET").unwrap();
        assert!(!axis.toggle);
    }

    #[test]
    fn descriptions_non_empty() {
        for ev in SIM_EVENT_CATALOG {
            assert!(
                !ev.description.is_empty(),
                "description empty for {}",
                ev.name
            );
        }
    }

    #[test]
    fn default_mapper_is_empty() {
        let mapper = SimEventMapper::default();
        assert_eq!(mapper.mapped_button_count(), 0);
    }
}
