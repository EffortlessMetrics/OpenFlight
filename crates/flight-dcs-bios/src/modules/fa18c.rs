// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! F/A-18C Hornet DCS-BIOS module definition.
//!
//! Pre-defines the most commonly used cockpit controls for the F/A-18C Hornet.
//! The base address for this module is `0x7400`.
//!
//! Control addresses and masks are based on the DCS-BIOS module for the
//! FA-18C_hornet aircraft.

use crate::controls::*;

/// Create the F/A-18C Hornet module with pre-defined controls.
///
/// Returns a [`DcsBiosModule`] containing 50+ commonly used controls.
#[must_use]
pub fn fa18c_module() -> DcsBiosModule {
    let mut m = DcsBiosModule::new(
        "FA-18C_hornet",
        0x7400,
        &["FA-18C_hornet", "EA-18G", "FA-18E", "FA-18F"],
    );

    // === Master Modes / Caution ===
    m.add_control(toggle_switch(
        "MASTER_ARM_SW",
        "Master Arm Panel",
        "Master Arm Switch, ARM/SAFE",
        0x740C,
        0x2000,
        13,
    ));
    m.add_control(push_button(
        "MASTER_MODE_AA",
        "Master Modes",
        "Master Mode A/A Button",
        0x740C,
        0x0200,
        9,
    ));
    m.add_control(push_button(
        "MASTER_MODE_AG",
        "Master Modes",
        "Master Mode A/G Button",
        0x740C,
        0x0400,
        10,
    ));
    m.add_control(push_button(
        "MASTER_CAUTION_RESET",
        "Caution/Advisory",
        "Master Caution Reset Button",
        0x7408,
        0x0400,
        10,
    ));
    m.add_control(indicator_light(
        "MASTER_CAUTION_LT",
        "Caution/Advisory",
        "Master Caution Light (yellow)",
        0x7408,
        0x0800,
        11,
    ));
    m.add_control(push_button(
        "EMER_JETT",
        "Master Arm Panel",
        "Emergency Jettison Button",
        0x740C,
        0x4000,
        14,
    ));

    // === UFC (Up Front Controller) Buttons ===
    m.add_control(push_button(
        "UFC_1",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 1",
        0x7410,
        0x0001,
        0,
    ));
    m.add_control(push_button(
        "UFC_2",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 2",
        0x7410,
        0x0002,
        1,
    ));
    m.add_control(push_button(
        "UFC_3",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 3",
        0x7410,
        0x0004,
        2,
    ));
    m.add_control(push_button(
        "UFC_4",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 4",
        0x7410,
        0x0008,
        3,
    ));
    m.add_control(push_button(
        "UFC_5",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 5",
        0x7410,
        0x0010,
        4,
    ));
    m.add_control(push_button(
        "UFC_6",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 6",
        0x7410,
        0x0020,
        5,
    ));
    m.add_control(push_button(
        "UFC_7",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 7",
        0x7410,
        0x0040,
        6,
    ));
    m.add_control(push_button(
        "UFC_8",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 8",
        0x7410,
        0x0080,
        7,
    ));
    m.add_control(push_button(
        "UFC_9",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 9",
        0x7410,
        0x0100,
        8,
    ));
    m.add_control(push_button(
        "UFC_0",
        "Up Front Controller (UFC)",
        "UFC Pushbutton 0",
        0x7410,
        0x0200,
        9,
    ));
    m.add_control(push_button(
        "UFC_ENT",
        "Up Front Controller (UFC)",
        "UFC ENT Button",
        0x7412,
        0x0001,
        0,
    ));
    m.add_control(push_button(
        "UFC_CLR",
        "Up Front Controller (UFC)",
        "UFC CLR Button",
        0x7412,
        0x0002,
        1,
    ));
    m.add_control(rotary_encoder(
        "UFC_COMM1_VOL",
        "Up Front Controller (UFC)",
        "UFC COMM 1 Volume",
        0x7414,
        0xFFFF,
        0,
        65535,
    ));
    m.add_control(rotary_encoder(
        "UFC_COMM2_VOL",
        "Up Front Controller (UFC)",
        "UFC COMM 2 Volume",
        0x7416,
        0xFFFF,
        0,
        65535,
    ));
    m.add_control(rotary_encoder(
        "UFC_BRT",
        "Up Front Controller (UFC)",
        "UFC Display Brightness",
        0x7418,
        0xFFFF,
        0,
        65535,
    ));

    // === UFC Scratchpad / Option Displays ===
    m.add_control(string_display(
        "UFC_SCRATCHPAD",
        "Up Front Controller (UFC)",
        "UFC Scratchpad Display",
        0x7424,
        8,
    ));
    m.add_control(string_display(
        "UFC_OPTION_CUEING_1",
        "Up Front Controller (UFC)",
        "UFC Option Cueing 1",
        0x742E,
        4,
    ));
    m.add_control(string_display(
        "UFC_OPTION_CUEING_2",
        "Up Front Controller (UFC)",
        "UFC Option Cueing 2",
        0x7432,
        4,
    ));
    m.add_control(string_display(
        "UFC_OPTION_CUEING_3",
        "Up Front Controller (UFC)",
        "UFC Option Cueing 3",
        0x7436,
        4,
    ));
    m.add_control(string_display(
        "UFC_OPTION_CUEING_4",
        "Up Front Controller (UFC)",
        "UFC Option Cueing 4",
        0x743A,
        4,
    ));
    m.add_control(string_display(
        "UFC_OPTION_CUEING_5",
        "Up Front Controller (UFC)",
        "UFC Option Cueing 5",
        0x743E,
        4,
    ));

    // === IFEI (Integrated Fuel/Engine Indicator) ===
    m.add_control(string_display(
        "IFEI_FUEL_DOWN",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Total Fuel Display",
        0x7480,
        6,
    ));
    m.add_control(string_display(
        "IFEI_RPM_L",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Left RPM Display",
        0x7486,
        6,
    ));
    m.add_control(string_display(
        "IFEI_RPM_R",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Right RPM Display",
        0x748C,
        6,
    ));
    m.add_control(string_display(
        "IFEI_TEMP_L",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Left Temp Display",
        0x7492,
        6,
    ));
    m.add_control(string_display(
        "IFEI_TEMP_R",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Right Temp Display",
        0x7498,
        6,
    ));
    m.add_control(string_display(
        "IFEI_FF_L",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Left Fuel Flow Display",
        0x749E,
        6,
    ));
    m.add_control(string_display(
        "IFEI_FF_R",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Right Fuel Flow Display",
        0x74A4,
        6,
    ));
    m.add_control(string_display(
        "IFEI_OIL_PRESS_L",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Left Oil Pressure",
        0x74AA,
        4,
    ));
    m.add_control(string_display(
        "IFEI_OIL_PRESS_R",
        "Integrated Fuel/Engine Indicator (IFEI)",
        "IFEI Right Oil Pressure",
        0x74AE,
        4,
    ));

    // === Landing Gear / Flaps / Hook ===
    m.add_control(toggle_switch(
        "GEAR_LEVER",
        "Landing Gear",
        "Landing Gear Handle, UP/DOWN",
        0x7462,
        0x0001,
        0,
    ));
    m.add_control(indicator_light(
        "GEAR_NOSE_LT",
        "Landing Gear",
        "Nose Gear Indicator Light (green)",
        0x7462,
        0x0004,
        2,
    ));
    m.add_control(indicator_light(
        "GEAR_LEFT_LT",
        "Landing Gear",
        "Left Gear Indicator Light (green)",
        0x7462,
        0x0008,
        3,
    ));
    m.add_control(indicator_light(
        "GEAR_RIGHT_LT",
        "Landing Gear",
        "Right Gear Indicator Light (green)",
        0x7462,
        0x0010,
        4,
    ));
    m.add_control(three_pos_switch(
        "FLAP_SW",
        "Flight Controls",
        "Flap Switch, AUTO/HALF/FULL",
        0x7464,
        0x0006,
        1,
    ));
    m.add_control(toggle_switch(
        "HOOK_LEVER",
        "Landing Gear",
        "Arresting Hook Handle",
        0x7462,
        0x0002,
        1,
    ));
    m.add_control(toggle_switch(
        "LAUNCH_BAR_SW",
        "Landing Gear",
        "Launch Bar Switch",
        0x7462,
        0x0020,
        5,
    ));

    // === Radio / TACAN ===
    m.add_control(rotary_encoder(
        "TACAN_CH_10",
        "TACAN Control Panel",
        "TACAN Channel Tens Selector",
        0x7466,
        0x001F,
        0,
        12,
    ));
    m.add_control(rotary_encoder(
        "TACAN_CH_1",
        "TACAN Control Panel",
        "TACAN Channel Ones Selector",
        0x7466,
        0x01E0,
        5,
        9,
    ));
    m.add_control(toggle_switch(
        "TACAN_XY",
        "TACAN Control Panel",
        "TACAN X/Y Channel Selector",
        0x7466,
        0x0200,
        9,
    ));
    m.add_control(selector(
        "TACAN_MODE",
        "TACAN Control Panel",
        "TACAN Mode Selector, OFF/REC/T-R/A-A/BCN",
        0x7468,
        0x000F,
        0,
        4,
    ));
    m.add_control(string_display(
        "COMM1_FREQ",
        "COMM1 Radio",
        "COMM 1 Frequency Display",
        0x74B2,
        8,
    ));
    m.add_control(string_display(
        "COMM2_FREQ",
        "COMM2 Radio",
        "COMM 2 Frequency Display",
        0x74BA,
        8,
    ));

    // === Lighting ===
    m.add_control(rotary_encoder(
        "CONSOLES_DIMMER",
        "Interior Lights",
        "Console Lights Brightness",
        0x7470,
        0xFFFF,
        0,
        65535,
    ));
    m.add_control(rotary_encoder(
        "INST_PNL_DIMMER",
        "Interior Lights",
        "Instrument Panel Lights Brightness",
        0x7472,
        0xFFFF,
        0,
        65535,
    ));
    m.add_control(rotary_encoder(
        "FLOOD_DIMMER",
        "Interior Lights",
        "Flood Lights Brightness",
        0x7474,
        0xFFFF,
        0,
        65535,
    ));
    m.add_control(three_pos_switch(
        "EXT_LIGHTS_FORMATION",
        "Exterior Lights",
        "Formation Lights Switch",
        0x7476,
        0x0006,
        1,
    ));
    m.add_control(three_pos_switch(
        "EXT_LIGHTS_POSITION",
        "Exterior Lights",
        "Position Lights Switch",
        0x7476,
        0x0018,
        3,
    ));
    m.add_control(toggle_switch(
        "EXT_LIGHTS_STROBE",
        "Exterior Lights",
        "Strobe Lights Switch",
        0x7476,
        0x0001,
        0,
    ));

    // === Engines ===
    m.add_control(three_pos_switch(
        "ENGINE_CRANK_L",
        "Engine",
        "Left Engine Crank Switch",
        0x7478,
        0x0003,
        0,
    ));
    m.add_control(three_pos_switch(
        "ENGINE_CRANK_R",
        "Engine",
        "Right Engine Crank Switch",
        0x7478,
        0x000C,
        2,
    ));

    // === HUD ===
    m.add_control(rotary_encoder(
        "HUD_SYM_BRT",
        "HUD Control Panel",
        "HUD Symbology Brightness",
        0x747A,
        0xFFFF,
        0,
        65535,
    ));
    m.add_control(selector(
        "HUD_SYM_REJ",
        "HUD Control Panel",
        "HUD Symbology Reject, NORM/REJ 1/REJ 2",
        0x747C,
        0x0003,
        0,
        2,
    ));

    // === Caution / Warning Lights ===
    m.add_control(indicator_light(
        "FIRE_LEFT_LT",
        "Caution/Advisory",
        "Left Fire Warning Light (red)",
        0x740E,
        0x0001,
        0,
    ));
    m.add_control(indicator_light(
        "FIRE_RIGHT_LT",
        "Caution/Advisory",
        "Right Fire Warning Light (red)",
        0x740E,
        0x0002,
        1,
    ));
    m.add_control(indicator_light(
        "CANOPY_LT",
        "Caution/Advisory",
        "Canopy Warning Light (yellow)",
        0x740E,
        0x0004,
        2,
    ));

    // === EWSP / Countermeasures ===
    m.add_control(selector(
        "CMSD_DISPENSE_SW",
        "Dispenser/EMC Panel",
        "Countermeasures Dispenser Switch, OFF/ON/BYPASS",
        0x747E,
        0x0003,
        0,
        2,
    ));
    m.add_control(push_button(
        "CMSD_JETT_SEL",
        "Dispenser/EMC Panel",
        "ECM JETT SEL Button",
        0x747E,
        0x0004,
        2,
    ));

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fa18c_module_loads() {
        let m = fa18c_module();
        assert_eq!(m.name, "FA-18C_hornet");
        assert_eq!(m.base_address, 0x7400);
        assert!(m.aircraft.contains(&"FA-18C_hornet".to_owned()));
    }

    #[test]
    fn fa18c_has_at_least_50_controls() {
        let m = fa18c_module();
        assert!(
            m.control_count() >= 50,
            "Expected >= 50 controls, got {}",
            m.control_count()
        );
    }

    #[test]
    fn fa18c_master_arm_exists() {
        let m = fa18c_module();
        let ctrl = m
            .get_control("MASTER_ARM_SW")
            .expect("MASTER_ARM_SW should exist");
        assert_eq!(ctrl.category, "Master Arm Panel");
        assert!(!ctrl.inputs.is_empty());
    }

    #[test]
    fn fa18c_ufc_buttons_exist() {
        let m = fa18c_module();
        for i in 0..=9 {
            let name = format!("UFC_{i}");
            assert!(m.get_control(&name).is_some(), "Missing control: {name}");
        }
        assert!(m.get_control("UFC_ENT").is_some());
        assert!(m.get_control("UFC_CLR").is_some());
    }

    #[test]
    fn fa18c_gear_controls_exist() {
        let m = fa18c_module();
        assert!(m.get_control("GEAR_LEVER").is_some());
        assert!(m.get_control("GEAR_NOSE_LT").is_some());
        assert!(m.get_control("GEAR_LEFT_LT").is_some());
        assert!(m.get_control("GEAR_RIGHT_LT").is_some());
    }

    #[test]
    fn fa18c_flap_and_hook_controls() {
        let m = fa18c_module();
        assert!(m.get_control("FLAP_SW").is_some());
        assert!(m.get_control("HOOK_LEVER").is_some());
        assert!(m.get_control("LAUNCH_BAR_SW").is_some());
    }

    #[test]
    fn fa18c_ifei_displays() {
        let m = fa18c_module();
        assert!(m.get_control("IFEI_FUEL_DOWN").is_some());
        assert!(m.get_control("IFEI_RPM_L").is_some());
        assert!(m.get_control("IFEI_RPM_R").is_some());
        assert!(m.get_control("IFEI_TEMP_L").is_some());
        assert!(m.get_control("IFEI_TEMP_R").is_some());
    }

    #[test]
    fn fa18c_tacan_controls() {
        let m = fa18c_module();
        assert!(m.get_control("TACAN_CH_10").is_some());
        assert!(m.get_control("TACAN_CH_1").is_some());
        assert!(m.get_control("TACAN_XY").is_some());
        assert!(m.get_control("TACAN_MODE").is_some());
    }

    #[test]
    fn fa18c_radio_frequencies() {
        let m = fa18c_module();
        assert!(m.get_control("COMM1_FREQ").is_some());
        assert!(m.get_control("COMM2_FREQ").is_some());
    }

    #[test]
    fn fa18c_has_multiple_categories() {
        let m = fa18c_module();
        let cats = m.categories();
        assert!(
            cats.len() >= 8,
            "Expected >= 8 categories, got {}",
            cats.len()
        );
    }

    #[test]
    fn fa18c_ufc_category_has_many_controls() {
        let m = fa18c_module();
        let ufc_controls = m.controls_in_category("Up Front Controller (UFC)");
        assert!(
            ufc_controls.len() >= 15,
            "Expected >= 15 UFC controls, got {}",
            ufc_controls.len()
        );
    }

    #[test]
    fn fa18c_scratchpad_display() {
        let m = fa18c_module();
        let (addr, len) = m
            .string_output("UFC_SCRATCHPAD")
            .expect("UFC_SCRATCHPAD should exist");
        assert_eq!(addr, 0x7424);
        assert_eq!(len, 8);
    }

    #[test]
    fn fa18c_master_arm_address_valid() {
        let m = fa18c_module();
        let addr = m.integer_output("MASTER_ARM_SW").unwrap();
        assert_eq!(addr.address, 0x740C);
        assert_eq!(addr.mask, 0x2000);
        assert_eq!(addr.shift, 13);
        assert_eq!(addr.max_value, 1);
    }
}
