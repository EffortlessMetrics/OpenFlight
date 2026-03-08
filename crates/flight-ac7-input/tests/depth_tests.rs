// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for flight-ac7-input: profile validation, render block
//! generation, managed-block patching, install workflow, and edge cases.

use flight_ac7_input::{
    Ac7InputError, Ac7InputProfile, ActionBinding, AxisBinding, RcMode,
    MANAGED_BLOCK_BEGIN, MANAGED_BLOCK_END,
    apply_profile_to_existing, install_profile, render_managed_block,
    steam_input_hint,
};
use std::fs;
use tempfile::tempdir;

// ---------------------------------------------------------------------------
// RcMode
// ---------------------------------------------------------------------------

#[test]
fn rc_mode_default_is_mode2() {
    assert_eq!(RcMode::default(), RcMode::Mode2);
}

#[test]
fn rc_mode_index_covers_all_variants() {
    let modes = [
        (RcMode::Mode1, 1),
        (RcMode::Mode2, 2),
        (RcMode::Mode3, 3),
        (RcMode::Mode4, 4),
    ];
    for (mode, expected) in modes {
        assert_eq!(mode.as_index(), expected, "{mode:?} index mismatch");
    }
}

#[test]
fn rc_mode_serialization_round_trip() {
    for mode in [RcMode::Mode1, RcMode::Mode2, RcMode::Mode3, RcMode::Mode4] {
        let json = serde_json::to_string(&mode).unwrap();
        let restored: RcMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, restored);
    }
}

// ---------------------------------------------------------------------------
// Profile validation — name
// ---------------------------------------------------------------------------

#[test]
fn validates_default_profile_ok() {
    assert!(Ac7InputProfile::default().validate().is_ok());
}

#[test]
fn rejects_empty_name() {
    let mut p = Ac7InputProfile::default();
    p.name = String::new();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_whitespace_only_name() {
    let mut p = Ac7InputProfile::default();
    p.name = "   ".to_string();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_name_with_double_quotes() {
    let mut p = Ac7InputProfile::default();
    p.name = r#"My "Profile""#.to_string();
    assert!(p.validate().is_err());
}

#[test]
fn accepts_name_with_single_quotes() {
    let mut p = Ac7InputProfile::default();
    p.name = "Player's Profile".to_string();
    assert!(p.validate().is_ok());
}

#[test]
fn accepts_unicode_name() {
    let mut p = Ac7InputProfile::default();
    p.name = "プロファイル".to_string();
    assert!(p.validate().is_ok());
}

// ---------------------------------------------------------------------------
// Profile validation — axis bindings
// ---------------------------------------------------------------------------

#[test]
fn rejects_empty_axis_name() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].axis_name = String::new();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_whitespace_axis_name() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].axis_name = "   ".to_string();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_empty_axis_key() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].key = String::new();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_axis_name_with_quotes() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].axis_name = r#"My"Axis"#.to_string();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_axis_key_with_quotes() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].key = r#"Key"Bad"#.to_string();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_scale_below_neg_two() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].scale = -2.1;
    assert!(p.validate().is_err());
}

#[test]
fn rejects_scale_above_two() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].scale = 2.1;
    assert!(p.validate().is_err());
}

#[test]
fn accepts_boundary_scale_values() {
    for scale in [-2.0_f32, 0.0, 2.0] {
        let mut p = Ac7InputProfile::default();
        p.axis_bindings[0].scale = scale;
        assert!(p.validate().is_ok(), "scale {scale} should be valid");
    }
}

#[test]
fn rejects_negative_deadzone() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].dead_zone = -0.01;
    assert!(p.validate().is_err());
}

#[test]
fn rejects_deadzone_above_one() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].dead_zone = 1.01;
    assert!(p.validate().is_err());
}

#[test]
fn accepts_boundary_deadzone_values() {
    for dz in [0.0_f32, 0.5, 1.0] {
        let mut p = Ac7InputProfile::default();
        p.axis_bindings[0].dead_zone = dz;
        assert!(p.validate().is_ok(), "dead_zone {dz} should be valid");
    }
}

#[test]
fn rejects_exponent_below_point_one() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].exponent = 0.09;
    assert!(p.validate().is_err());
}

#[test]
fn rejects_exponent_above_five() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings[0].exponent = 5.1;
    assert!(p.validate().is_err());
}

#[test]
fn accepts_boundary_exponent_values() {
    for exp in [0.1_f32, 1.0, 5.0] {
        let mut p = Ac7InputProfile::default();
        p.axis_bindings[0].exponent = exp;
        assert!(p.validate().is_ok(), "exponent {exp} should be valid");
    }
}

// ---------------------------------------------------------------------------
// Profile validation — action bindings
// ---------------------------------------------------------------------------

#[test]
fn rejects_empty_action_name() {
    let mut p = Ac7InputProfile::default();
    p.action_bindings[0].action_name = String::new();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_whitespace_action_name() {
    let mut p = Ac7InputProfile::default();
    p.action_bindings[0].action_name = " \t ".to_string();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_empty_action_key() {
    let mut p = Ac7InputProfile::default();
    p.action_bindings[0].key = String::new();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_action_name_with_quotes() {
    let mut p = Ac7InputProfile::default();
    p.action_bindings[0].action_name = r#"Fire"Gun"#.to_string();
    assert!(p.validate().is_err());
}

#[test]
fn rejects_action_key_with_quotes() {
    let mut p = Ac7InputProfile::default();
    p.action_bindings[0].key = r#"Button"0"#.to_string();
    assert!(p.validate().is_err());
}

// ---------------------------------------------------------------------------
// Profile validation — profile with no bindings
// ---------------------------------------------------------------------------

#[test]
fn accepts_profile_with_no_bindings() {
    let p = Ac7InputProfile {
        name: "EmptyProfile".to_string(),
        rc_mode: RcMode::Mode1,
        enable_joystick: false,
        steam_input_disabled_hint: false,
        axis_bindings: vec![],
        action_bindings: vec![],
    };
    assert!(p.validate().is_ok());
}

// ---------------------------------------------------------------------------
// Render managed block
// ---------------------------------------------------------------------------

#[test]
fn render_block_starts_and_ends_with_markers() {
    let block = render_managed_block(&Ac7InputProfile::default()).unwrap();
    assert!(block.starts_with(MANAGED_BLOCK_BEGIN));
    assert!(block.trim_end().ends_with(MANAGED_BLOCK_END));
}

#[test]
fn render_block_contains_profile_name() {
    let mut p = Ac7InputProfile::default();
    p.name = "CustomHOTAS".to_string();
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains(r#"ProfileName="CustomHOTAS""#));
}

#[test]
fn render_block_contains_rc_mode() {
    for mode in [RcMode::Mode1, RcMode::Mode2, RcMode::Mode3, RcMode::Mode4] {
        let mut p = Ac7InputProfile::default();
        p.rc_mode = mode;
        let block = render_managed_block(&p).unwrap();
        assert!(
            block.contains(&format!("RCMode={}", mode.as_index())),
            "mode {:?} not rendered correctly",
            mode
        );
    }
}

#[test]
fn render_block_joystick_true() {
    let p = Ac7InputProfile::default();
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains("EnableJoystick=True"));
}

#[test]
fn render_block_joystick_false() {
    let mut p = Ac7InputProfile::default();
    p.enable_joystick = false;
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains("EnableJoystick=False"));
}

#[test]
fn render_block_steam_input_hint_true() {
    let p = Ac7InputProfile::default();
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains("SteamInputMustBeDisabled=True"));
}

#[test]
fn render_block_steam_input_hint_false() {
    let mut p = Ac7InputProfile::default();
    p.steam_input_disabled_hint = false;
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains("SteamInputMustBeDisabled=False"));
}

#[test]
fn render_block_contains_axis_config_for_each_binding() {
    let p = Ac7InputProfile::default();
    let block = render_managed_block(&p).unwrap();
    for axis in &p.axis_bindings {
        assert!(
            block.contains(&format!("AxisName=\"{}\"", axis.axis_name)),
            "missing axis: {}",
            axis.axis_name
        );
        assert!(
            block.contains(&format!("AxisKeyName=\"{}\"", axis.key)),
            "missing axis key: {}",
            axis.key
        );
    }
}

#[test]
fn render_block_contains_action_mappings() {
    let p = Ac7InputProfile::default();
    let block = render_managed_block(&p).unwrap();
    for action in &p.action_bindings {
        assert!(
            block.contains(&format!("ActionName=\"{}\"", action.action_name)),
            "missing action: {}",
            action.action_name
        );
    }
}

#[test]
fn render_block_deadzone_precision_four_decimals() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings = vec![AxisBinding {
        axis_name: "Test".to_string(),
        key: "Joystick_Axis0".to_string(),
        scale: 1.0,
        dead_zone: 0.03,
        exponent: 1.0,
        invert: false,
    }];
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains("DeadZone=0.0300"), "block: {block}");
}

#[test]
fn render_block_invert_true() {
    let mut p = Ac7InputProfile::default();
    p.axis_bindings = vec![AxisBinding {
        axis_name: "Pitch".to_string(),
        key: "Joystick_Axis1".to_string(),
        scale: -1.0,
        dead_zone: 0.0,
        exponent: 1.0,
        invert: true,
    }];
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains("bInvert=True"));
}

#[test]
fn render_block_invert_false() {
    let p = Ac7InputProfile::default();
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains("bInvert=False"));
}

#[test]
fn render_block_empty_bindings() {
    let p = Ac7InputProfile {
        name: "Minimal".to_string(),
        axis_bindings: vec![],
        action_bindings: vec![],
        ..Ac7InputProfile::default()
    };
    let block = render_managed_block(&p).unwrap();
    assert!(block.contains(MANAGED_BLOCK_BEGIN));
    assert!(block.contains(MANAGED_BLOCK_END));
    assert!(!block.contains("+AxisMappings"));
    assert!(!block.contains("+ActionMappings"));
}

#[test]
fn render_block_rejects_invalid_profile() {
    let mut p = Ac7InputProfile::default();
    p.name = String::new();
    assert!(render_managed_block(&p).is_err());
}

#[test]
fn render_block_contains_generator_comment() {
    let block = render_managed_block(&Ac7InputProfile::default()).unwrap();
    assert!(block.contains("Generated by Flight Hub"));
}

#[test]
fn render_block_contains_input_settings_section() {
    let block = render_managed_block(&Ac7InputProfile::default()).unwrap();
    assert!(block.contains("[/Script/Engine.InputSettings]"));
}

// ---------------------------------------------------------------------------
// apply_profile_to_existing
// ---------------------------------------------------------------------------

#[test]
fn apply_to_empty_content() {
    let result = apply_profile_to_existing("", &Ac7InputProfile::default()).unwrap();
    assert!(result.contains(MANAGED_BLOCK_BEGIN));
    assert!(result.contains("EnableJoystick=True"));
}

#[test]
fn apply_preserves_user_content_before_block() {
    let existing = "[SomeSection]\nUserKey=Value\n";
    let result = apply_profile_to_existing(existing, &Ac7InputProfile::default()).unwrap();
    assert!(result.contains("[SomeSection]"));
    assert!(result.contains("UserKey=Value"));
    assert!(result.contains(MANAGED_BLOCK_BEGIN));
}

#[test]
fn apply_replaces_old_managed_block() {
    let existing = format!(
        "[Header]\nFoo=Bar\n{}\nOldStuff\n{}\n[Footer]\nBaz=1\n",
        MANAGED_BLOCK_BEGIN, MANAGED_BLOCK_END
    );
    let result = apply_profile_to_existing(&existing, &Ac7InputProfile::default()).unwrap();
    assert_eq!(result.matches(MANAGED_BLOCK_BEGIN).count(), 1);
    assert_eq!(result.matches(MANAGED_BLOCK_END).count(), 1);
    assert!(!result.contains("OldStuff"));
    assert!(result.contains("Foo=Bar"));
    assert!(result.contains("Baz=1"));
}

#[test]
fn apply_enforces_enable_joystick_true() {
    let existing = "[Joystick]\nEnableJoystick=False\n";
    let result = apply_profile_to_existing(existing, &Ac7InputProfile::default()).unwrap();
    assert!(!result.contains("EnableJoystick=False"));
    assert!(result.contains("EnableJoystick=True"));
}

#[test]
fn apply_idempotent_double_application() {
    let profile = Ac7InputProfile::default();
    let first = apply_profile_to_existing("", &profile).unwrap();
    let second = apply_profile_to_existing(&first, &profile).unwrap();
    assert_eq!(second.matches(MANAGED_BLOCK_BEGIN).count(), 1);
    assert_eq!(second.matches(MANAGED_BLOCK_END).count(), 1);
}

#[test]
fn apply_adds_joystick_section_when_missing() {
    let existing = "[OtherSection]\nKey=Value\n";
    let result = apply_profile_to_existing(existing, &Ac7InputProfile::default()).unwrap();
    assert!(result.contains("[Joystick]"));
    assert!(result.contains("EnableJoystick=True"));
}

#[test]
fn apply_replaces_enable_joystick_in_existing_joystick_section() {
    let existing = "[Joystick]\nEnableJoystick=False\nSomeSetting=1\n";
    let result = apply_profile_to_existing(existing, &Ac7InputProfile::default()).unwrap();
    // EnableJoystick should be True, SomeSetting preserved
    assert!(result.contains("EnableJoystick=True"));
    assert!(result.contains("SomeSetting=1"));
    // No double [Joystick] sections from the upsert
    let joystick_count = result.matches("[Joystick]").count();
    assert_eq!(joystick_count, 1, "should only have one [Joystick] section");
}

// ---------------------------------------------------------------------------
// install_profile
// ---------------------------------------------------------------------------

#[test]
fn install_creates_file_in_new_dir() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("sub").join("Input.ini");
    let result = install_profile(&path, &Ac7InputProfile::default(), false).unwrap();
    assert!(result.input_ini_path.exists());
    assert!(result.backup_path.is_none());
    assert!(result.bytes_written > 0);

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains(MANAGED_BLOCK_BEGIN));
}

#[test]
fn install_creates_backup_when_file_exists() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Input.ini");
    fs::write(&path, "[Existing]\nKey=Val\n").unwrap();

    let result = install_profile(&path, &Ac7InputProfile::default(), true).unwrap();
    let backup = result.backup_path.as_ref().expect("backup should exist");
    assert!(backup.exists());

    let backup_content = fs::read_to_string(backup).unwrap();
    assert!(backup_content.contains("[Existing]"));
}

#[test]
fn install_no_backup_when_requested_false() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Input.ini");
    fs::write(&path, "old").unwrap();

    let result = install_profile(&path, &Ac7InputProfile::default(), false).unwrap();
    assert!(result.backup_path.is_none());
}

#[test]
fn install_no_backup_when_file_doesnt_exist() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Input.ini");

    let result = install_profile(&path, &Ac7InputProfile::default(), true).unwrap();
    assert!(result.backup_path.is_none());
}

#[test]
fn install_replaces_existing_managed_block() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Input.ini");

    // First install
    install_profile(&path, &Ac7InputProfile::default(), false).unwrap();

    // Second install with different profile name
    let mut profile = Ac7InputProfile::default();
    profile.name = "UpdatedProfile".to_string();
    install_profile(&path, &profile, false).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content.matches(MANAGED_BLOCK_BEGIN).count(), 1);
    assert!(content.contains("UpdatedProfile"));
}

#[test]
fn install_rejects_invalid_profile() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Input.ini");
    let mut profile = Ac7InputProfile::default();
    profile.name = String::new();
    assert!(install_profile(&path, &profile, false).is_err());
}

#[test]
fn install_bytes_written_matches_content_length() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Input.ini");
    let result = install_profile(&path, &Ac7InputProfile::default(), false).unwrap();
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(result.bytes_written, content.len());
}

// ---------------------------------------------------------------------------
// Managed block markers
// ---------------------------------------------------------------------------

#[test]
fn managed_block_markers_are_comments() {
    assert!(MANAGED_BLOCK_BEGIN.starts_with(';'));
    assert!(MANAGED_BLOCK_END.starts_with(';'));
}

#[test]
fn managed_block_begin_end_are_different() {
    assert_ne!(MANAGED_BLOCK_BEGIN, MANAGED_BLOCK_END);
}

// ---------------------------------------------------------------------------
// steam_input_hint
// ---------------------------------------------------------------------------

#[test]
fn steam_input_hint_mentions_steam_input() {
    let hint = steam_input_hint();
    assert!(hint.contains("Steam Input"));
}

#[test]
fn steam_input_hint_mentions_ace_combat() {
    let hint = steam_input_hint();
    assert!(hint.contains("ACE COMBAT 7"));
}

// ---------------------------------------------------------------------------
// Profile serialization round-trip
// ---------------------------------------------------------------------------

#[test]
fn profile_serde_round_trip_default() {
    let original = Ac7InputProfile::default();
    let json = serde_json::to_string(&original).unwrap();
    let restored: Ac7InputProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn profile_serde_round_trip_custom() {
    let original = Ac7InputProfile {
        name: "MyCustom".to_string(),
        rc_mode: RcMode::Mode3,
        enable_joystick: false,
        steam_input_disabled_hint: false,
        axis_bindings: vec![AxisBinding {
            axis_name: "CustomAxis".to_string(),
            key: "Joystick_Axis5".to_string(),
            scale: -0.5,
            dead_zone: 0.1,
            exponent: 2.0,
            invert: true,
        }],
        action_bindings: vec![ActionBinding {
            action_name: "CustomAction".to_string(),
            key: "Joystick_Button10".to_string(),
        }],
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: Ac7InputProfile = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

// ---------------------------------------------------------------------------
// AxisBinding / ActionBinding
// ---------------------------------------------------------------------------

#[test]
fn axis_binding_clone_equality() {
    let binding = AxisBinding {
        axis_name: "Pitch".to_string(),
        key: "Joystick_Axis1".to_string(),
        scale: -1.0,
        dead_zone: 0.03,
        exponent: 1.0,
        invert: false,
    };
    assert_eq!(binding, binding.clone());
}

#[test]
fn action_binding_clone_equality() {
    let binding = ActionBinding {
        action_name: "FireGun".to_string(),
        key: "Joystick_Button0".to_string(),
    };
    assert_eq!(binding, binding.clone());
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[test]
fn error_display_invalid_profile() {
    let err = Ac7InputError::InvalidProfile("test reason".to_string());
    let msg = err.to_string();
    assert!(msg.contains("test reason"), "got: {msg}");
    assert!(msg.contains("profile validation failed"), "got: {msg}");
}

#[test]
fn error_display_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
    let err = Ac7InputError::Io(io_err);
    let msg = err.to_string();
    assert!(msg.contains("io error"), "got: {msg}");
}

// ---------------------------------------------------------------------------
// Default profile content
// ---------------------------------------------------------------------------

#[test]
fn default_profile_has_four_axis_bindings() {
    let p = Ac7InputProfile::default();
    assert_eq!(p.axis_bindings.len(), 4);
    let names: Vec<&str> = p.axis_bindings.iter().map(|a| a.axis_name.as_str()).collect();
    assert!(names.contains(&"Pitch"));
    assert!(names.contains(&"Roll"));
    assert!(names.contains(&"Yaw"));
    assert!(names.contains(&"Throttle"));
}

#[test]
fn default_profile_has_four_action_bindings() {
    let p = Ac7InputProfile::default();
    assert_eq!(p.action_bindings.len(), 4);
    let names: Vec<&str> = p
        .action_bindings
        .iter()
        .map(|a| a.action_name.as_str())
        .collect();
    assert!(names.contains(&"FireGun"));
    assert!(names.contains(&"FireMissile"));
    assert!(names.contains(&"ChangeWeapon"));
    assert!(names.contains(&"TargetSwitch"));
}

#[test]
fn default_profile_joystick_enabled() {
    let p = Ac7InputProfile::default();
    assert!(p.enable_joystick);
}

#[test]
fn default_profile_steam_hint_enabled() {
    let p = Ac7InputProfile::default();
    assert!(p.steam_input_disabled_hint);
}

// ---------------------------------------------------------------------------
// Multiple axis bindings validation
// ---------------------------------------------------------------------------

#[test]
fn rejects_second_axis_with_bad_scale() {
    let mut p = Ac7InputProfile::default();
    // First axis is fine, second has bad scale
    p.axis_bindings.push(AxisBinding {
        axis_name: "Extra".to_string(),
        key: "Joystick_Axis9".to_string(),
        scale: 10.0,
        dead_zone: 0.0,
        exponent: 1.0,
        invert: false,
    });
    assert!(p.validate().is_err());
}

#[test]
fn rejects_second_action_with_empty_key() {
    let mut p = Ac7InputProfile::default();
    p.action_bindings.push(ActionBinding {
        action_name: "ExtraAction".to_string(),
        key: String::new(),
    });
    assert!(p.validate().is_err());
}

// ---------------------------------------------------------------------------
// Complex integration: full workflow
// ---------------------------------------------------------------------------

#[test]
fn full_workflow_install_update_verify() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("Config").join("Input.ini");

    // Initial install
    let profile1 = Ac7InputProfile::default();
    let r1 = install_profile(&path, &profile1, false).unwrap();
    assert!(r1.input_ini_path.exists());

    // Update with different RC mode
    let mut profile2 = Ac7InputProfile::default();
    profile2.rc_mode = RcMode::Mode4;
    profile2.name = "HOTAS v2".to_string();
    let r2 = install_profile(&path, &profile2, true).unwrap();
    assert!(r2.backup_path.is_some());

    let final_content = fs::read_to_string(&path).unwrap();
    assert_eq!(final_content.matches(MANAGED_BLOCK_BEGIN).count(), 1);
    assert!(final_content.contains("RCMode=4"));
    assert!(final_content.contains(r#"ProfileName="HOTAS v2""#));
    assert!(final_content.contains("EnableJoystick=True"));
}
