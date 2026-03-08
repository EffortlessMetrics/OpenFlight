// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for DCS-BIOS aircraft module support.
//!
//! Covers per-aircraft control definitions, category filtering, variable
//! lookup, and module-discovery helpers.

use std::fs;

use flight_dcs_modules::{ControlType, DcsControl, DcsModule, ModuleLoader};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers — reusable TOML content for each aircraft
// ---------------------------------------------------------------------------

fn fa18c_toml() -> &'static str {
    r#"
aircraft = "F/A-18C"
version = "2.8"
description = "McDonnell Douglas F/A-18C Hornet"
axis_count = 6
throttle_range = [0.0, 1.0]
stick_throw = 45.0
quirks = ["twin-throttle", "catapult-bar"]

[[controls]]
name = "UFC_SCRATCHPAD"
category = "UFC"
control_type = "display"
address = 29696
description = "Up Front Controller scratch pad display"

[[controls]]
name = "DDI_L_PB_01"
category = "DDI"
control_type = "button"
address = 29700
description = "Left DDI pushbutton 1"

[[controls]]
name = "FLIR_GAIN"
category = "FLIR"
control_type = "axis"
address = 29704
description = "FLIR gain knob"

[[controls]]
name = "WPN_STA_2"
category = "Weapons"
control_type = "selector"
address = 29708
description = "Station 2 weapon selector"

[[controls]]
name = "MFD_L_PB_05"
category = "MFD"
control_type = "button"
address = 29712
description = "Left MFD pushbutton 5"
"#
}

fn a10c_toml() -> &'static str {
    r#"
aircraft = "A-10C"
version = "2.8"
description = "Fairchild Republic A-10C Thunderbolt II"
axis_count = 7
throttle_range = [0.0, 1.0]
stick_throw = 50.0
quirks = ["twin-throttle", "gun-trigger"]

[[controls]]
name = "CDU_DISPLAY"
category = "CDU"
control_type = "display"
address = 30000
description = "CDU main display"

[[controls]]
name = "UFC_1"
category = "UFC"
control_type = "button"
address = 30004
description = "UFC digit 1 button"

[[controls]]
name = "MFCD_L_PB_03"
category = "MFCD"
control_type = "button"
address = 30008
description = "Left MFCD pushbutton 3"

[[controls]]
name = "CMSC_JETT"
category = "CMSC"
control_type = "button"
address = 30012
description = "CMSC jettison button"

[[controls]]
name = "AAP_STEER"
category = "AAP"
control_type = "selector"
address = 30016
description = "AAP steer-point selector"
"#
}

fn f16c_toml() -> &'static str {
    r#"
aircraft = "F-16C"
version = "4.3"
description = "General Dynamics F-16C Fighting Falcon"
axis_count = 5
throttle_range = [0.0, 1.0]
stick_throw = 30.0
quirks = ["side-stick", "fbw"]

[[controls]]
name = "ICP_DED_LINE1"
category = "ICP"
control_type = "display"
address = 31000
description = "ICP Data Entry Display line 1"

[[controls]]
name = "MFD_L_OSB_01"
category = "MFD"
control_type = "button"
address = 31004
description = "Left MFD OSB 1"

[[controls]]
name = "SMS_STA_SEL"
category = "SMS"
control_type = "selector"
address = 31008
description = "SMS station select knob"

[[controls]]
name = "UHF_FREQ"
category = "Radio"
control_type = "display"
address = 31012
description = "UHF radio frequency display"

[[controls]]
name = "HMCS_BRIGHT"
category = "HMCS"
control_type = "axis"
address = 31016
description = "HMCS brightness knob"
"#
}

fn ah64d_toml() -> &'static str {
    r#"
aircraft = "AH-64D"
version = "1.0"
description = "Boeing AH-64D Apache Longbow"
axis_count = 4
throttle_range = [0.0, 1.0]
stick_throw = 40.0
quirks = ["collective", "cyclic", "anti-torque"]

[[controls]]
name = "TADS_FOV"
category = "TADS"
control_type = "selector"
address = 32000
description = "TADS field-of-view selector"

[[controls]]
name = "MPD_L_T1"
category = "MPD"
control_type = "button"
address = 32004
description = "Left MPD top button 1"

[[controls]]
name = "KU_A"
category = "KU"
control_type = "button"
address = 32008
description = "Keyboard unit key A"

[[controls]]
name = "WPN_MSL_TYPE"
category = "Weapons"
control_type = "selector"
address = 32012
description = "Weapons page missile type selector"

[[controls]]
name = "RADIO_PRESET_1"
category = "Radio"
control_type = "selector"
address = 32016
description = "Radio preset channel 1"
"#
}

/// Write all four aircraft modules to a temp directory and load them.
fn load_all_modules() -> (TempDir, ModuleLoader) {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("fa-18c.toml"), fa18c_toml()).unwrap();
    fs::write(dir.path().join("a-10c.toml"), a10c_toml()).unwrap();
    fs::write(dir.path().join("f-16c.toml"), f16c_toml()).unwrap();
    fs::write(dir.path().join("ah-64d.toml"), ah64d_toml()).unwrap();
    let mut loader = ModuleLoader::new();
    loader.load_from_dir(dir.path()).unwrap();
    (dir, loader)
}

// ===================================================================
// F/A-18C Hornet — 5 tests
// ===================================================================

#[test]
fn fa18c_ufc_display_present() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F/A-18C").unwrap();
    let ctrl = m.find_control("UFC_SCRATCHPAD").expect("UFC display");
    assert_eq!(ctrl.control_type, ControlType::Display);
    assert_eq!(ctrl.category, "UFC");
    assert_eq!(ctrl.address, 29696);
}

#[test]
fn fa18c_ddi_button_present() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F/A-18C").unwrap();
    let ctrl = m.find_control("DDI_L_PB_01").expect("DDI button");
    assert_eq!(ctrl.control_type, ControlType::Button);
    assert_eq!(ctrl.category, "DDI");
}

#[test]
fn fa18c_flir_controls() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F/A-18C").unwrap();
    let flir = m.controls_by_category("FLIR");
    assert_eq!(flir.len(), 1);
    assert_eq!(flir[0].name, "FLIR_GAIN");
    assert_eq!(flir[0].control_type, ControlType::Axis);
}

#[test]
fn fa18c_weapons_station_selector() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F/A-18C").unwrap();
    let ctrl = m.find_control("WPN_STA_2").expect("weapon station");
    assert_eq!(ctrl.control_type, ControlType::Selector);
    assert_eq!(ctrl.category, "Weapons");
}

#[test]
fn fa18c_mfd_display_button() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F/A-18C").unwrap();
    let mfd = m.controls_by_category("MFD");
    assert_eq!(mfd.len(), 1);
    assert_eq!(mfd[0].name, "MFD_L_PB_05");
    assert_eq!(mfd[0].control_type, ControlType::Button);
}

// ===================================================================
// A-10C Warthog — 5 tests
// ===================================================================

#[test]
fn a10c_cdu_display() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("A-10C").unwrap();
    let ctrl = m.find_control("CDU_DISPLAY").expect("CDU display");
    assert_eq!(ctrl.control_type, ControlType::Display);
    assert_eq!(ctrl.address, 30000);
}

#[test]
fn a10c_ufc_panel() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("A-10C").unwrap();
    let ufc = m.controls_by_category("UFC");
    assert_eq!(ufc.len(), 1);
    assert_eq!(ufc[0].name, "UFC_1");
    assert_eq!(ufc[0].control_type, ControlType::Button);
}

#[test]
fn a10c_mfcd_buttons() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("A-10C").unwrap();
    let ctrl = m.find_control("MFCD_L_PB_03").expect("MFCD button");
    assert_eq!(ctrl.control_type, ControlType::Button);
    assert_eq!(ctrl.category, "MFCD");
}

#[test]
fn a10c_cmsc_panel() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("A-10C").unwrap();
    let cmsc = m.controls_by_category("CMSC");
    assert_eq!(cmsc.len(), 1);
    assert_eq!(cmsc[0].name, "CMSC_JETT");
}

#[test]
fn a10c_aap_panel() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("A-10C").unwrap();
    let ctrl = m.find_control("AAP_STEER").expect("AAP selector");
    assert_eq!(ctrl.control_type, ControlType::Selector);
    assert_eq!(ctrl.category, "AAP");
    assert_eq!(ctrl.address, 30016);
}

// ===================================================================
// F-16C Viper — 5 tests
// ===================================================================

#[test]
fn f16c_icp_ded_display() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F-16C").unwrap();
    let ctrl = m.find_control("ICP_DED_LINE1").expect("ICP/DED display");
    assert_eq!(ctrl.control_type, ControlType::Display);
    assert_eq!(ctrl.category, "ICP");
    assert_eq!(ctrl.address, 31000);
}

#[test]
fn f16c_mfd_buttons() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F-16C").unwrap();
    let mfd = m.controls_by_category("MFD");
    assert_eq!(mfd.len(), 1);
    assert_eq!(mfd[0].name, "MFD_L_OSB_01");
    assert_eq!(mfd[0].control_type, ControlType::Button);
}

#[test]
fn f16c_sms_panel() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F-16C").unwrap();
    let ctrl = m.find_control("SMS_STA_SEL").expect("SMS selector");
    assert_eq!(ctrl.control_type, ControlType::Selector);
    assert_eq!(ctrl.category, "SMS");
}

#[test]
fn f16c_uhf_vhf_radio() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F-16C").unwrap();
    let radios = m.controls_by_category("Radio");
    assert_eq!(radios.len(), 1);
    assert_eq!(radios[0].name, "UHF_FREQ");
    assert_eq!(radios[0].control_type, ControlType::Display);
}

#[test]
fn f16c_hmcs_controls() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("F-16C").unwrap();
    let ctrl = m.find_control("HMCS_BRIGHT").expect("HMCS brightness");
    assert_eq!(ctrl.control_type, ControlType::Axis);
    assert_eq!(ctrl.category, "HMCS");
    assert_eq!(ctrl.address, 31016);
}

// ===================================================================
// AH-64D Apache — 5 tests
// ===================================================================

#[test]
fn ah64d_tads_pnvs_controls() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("AH-64D").unwrap();
    let tads = m.controls_by_category("TADS");
    assert_eq!(tads.len(), 1);
    assert_eq!(tads[0].name, "TADS_FOV");
    assert_eq!(tads[0].control_type, ControlType::Selector);
}

#[test]
fn ah64d_mpd_pages() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("AH-64D").unwrap();
    let ctrl = m.find_control("MPD_L_T1").expect("MPD button");
    assert_eq!(ctrl.control_type, ControlType::Button);
    assert_eq!(ctrl.category, "MPD");
    assert_eq!(ctrl.address, 32004);
}

#[test]
fn ah64d_keyboard_unit() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("AH-64D").unwrap();
    let ku = m.controls_by_category("KU");
    assert_eq!(ku.len(), 1);
    assert_eq!(ku[0].name, "KU_A");
    assert_eq!(ku[0].control_type, ControlType::Button);
}

#[test]
fn ah64d_weapons_page() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("AH-64D").unwrap();
    let ctrl = m.find_control("WPN_MSL_TYPE").expect("weapons selector");
    assert_eq!(ctrl.control_type, ControlType::Selector);
    assert_eq!(ctrl.category, "Weapons");
}

#[test]
fn ah64d_radio_presets() {
    let (_dir, loader) = load_all_modules();
    let m = loader.get("AH-64D").unwrap();
    let ctrl = m.find_control("RADIO_PRESET_1").expect("radio preset");
    assert_eq!(ctrl.control_type, ControlType::Selector);
    assert_eq!(ctrl.category, "Radio");
    assert_eq!(ctrl.address, 32016);
}

// ===================================================================
// Module discovery — 5 tests
// ===================================================================

#[test]
fn discovery_enumerate_available_modules() {
    let (_dir, loader) = load_all_modules();
    let names = loader.aircraft_names();
    assert_eq!(names.len(), 4);
    assert!(names.contains(&"F/A-18C"));
    assert!(names.contains(&"A-10C"));
    assert!(names.contains(&"F-16C"));
    assert!(names.contains(&"AH-64D"));
}

#[test]
fn discovery_version_matching() {
    let (_dir, loader) = load_all_modules();
    let v28 = loader.modules_with_version("2.8");
    let mut names: Vec<&str> = v28.iter().map(|m| m.aircraft.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["A-10C", "F/A-18C"]);

    let v43 = loader.modules_with_version("4.3");
    assert_eq!(v43.len(), 1);
    assert_eq!(v43[0].aircraft, "F-16C");

    let none = loader.modules_with_version("99.0");
    assert!(none.is_empty());
}

#[test]
fn discovery_variable_lookup_by_name() {
    let (_dir, loader) = load_all_modules();
    // Look up a control across different aircraft by iterating.
    let found: Vec<&str> = loader
        .all_modules()
        .filter_map(|m| m.find_control("UFC_SCRATCHPAD").map(|_| m.aircraft.as_str()))
        .collect();
    assert_eq!(found, vec!["F/A-18C"]);

    // Non-existent control should not appear in any module.
    let missing: Vec<&str> = loader
        .all_modules()
        .filter_map(|m| m.find_control("DOES_NOT_EXIST").map(|_| m.aircraft.as_str()))
        .collect();
    assert!(missing.is_empty());
}

#[test]
fn discovery_category_filtering() {
    let (_dir, loader) = load_all_modules();
    // Both F-16C and AH-64D have a "Radio" category.
    let mut with_radio: Vec<&str> = loader
        .all_modules()
        .filter(|m| m.categories().contains(&"Radio"))
        .map(|m| m.aircraft.as_str())
        .collect();
    with_radio.sort_unstable();
    assert_eq!(with_radio, vec!["AH-64D", "F-16C"]);

    // Only F/A-18C has a "DDI" category.
    let with_ddi: Vec<&str> = loader
        .all_modules()
        .filter(|m| m.categories().contains(&"DDI"))
        .map(|m| m.aircraft.as_str())
        .collect();
    assert_eq!(with_ddi, vec!["F/A-18C"]);
}

#[test]
fn discovery_module_metadata() {
    let (_dir, loader) = load_all_modules();
    let hornet = loader.get("F/A-18C").unwrap();
    assert_eq!(hornet.version.as_deref(), Some("2.8"));
    assert_eq!(
        hornet.description.as_deref(),
        Some("McDonnell Douglas F/A-18C Hornet"),
    );
    assert_eq!(hornet.controls.len(), 5);

    let apache = loader.get("AH-64D").unwrap();
    assert_eq!(apache.version.as_deref(), Some("1.0"));
    assert_eq!(
        apache.description.as_deref(),
        Some("Boeing AH-64D Apache Longbow"),
    );
    assert_eq!(apache.quirks, vec!["collective", "cyclic", "anti-torque"]);
}

// ===================================================================
// Extra depth — serde roundtrip with controls
// ===================================================================

#[test]
fn control_type_serde_roundtrip() {
    let ctrl = DcsControl {
        name: "TEST_CTRL".to_owned(),
        category: "Test".to_owned(),
        control_type: ControlType::Toggle,
        address: 0xFFFF,
        description: "test toggle".to_owned(),
    };
    let serialized = toml::to_string(&ctrl).unwrap();
    let restored: DcsControl = toml::from_str(&serialized).unwrap();
    assert_eq!(restored, ctrl);
}

#[test]
fn module_with_controls_serde_roundtrip() {
    let module = DcsModule {
        aircraft: "Test-AC".to_owned(),
        axis_count: 3,
        throttle_range: [0.0, 0.8],
        stick_throw: 25.0,
        quirks: vec!["test".to_owned()],
        version: Some("1.0".to_owned()),
        description: Some("Test aircraft".to_owned()),
        controls: vec![DcsControl {
            name: "BTN_1".to_owned(),
            category: "Panel".to_owned(),
            control_type: ControlType::Button,
            address: 100,
            description: "Button 1".to_owned(),
        }],
    };
    let serialized = toml::to_string(&module).unwrap();
    let restored: DcsModule = toml::from_str(&serialized).unwrap();
    assert_eq!(restored.aircraft, "Test-AC");
    assert_eq!(restored.controls.len(), 1);
    assert_eq!(restored.controls[0], module.controls[0]);
}
