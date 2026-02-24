// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! AC7 Input.ini generation and managed patching.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Start marker for the managed block in Input.ini.
pub const MANAGED_BLOCK_BEGIN: &str = "; >>> FLIGHT HUB AC7 MANAGED BLOCK >>>";
/// End marker for the managed block in Input.ini.
pub const MANAGED_BLOCK_END: &str = "; <<< FLIGHT HUB AC7 MANAGED BLOCK <<<";

/// AC7 RC control mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RcMode {
    Mode1,
    #[default]
    Mode2,
    Mode3,
    Mode4,
}

impl RcMode {
    /// Numeric mode value used in generated config.
    pub const fn as_index(self) -> u8 {
        match self {
            RcMode::Mode1 => 1,
            RcMode::Mode2 => 2,
            RcMode::Mode3 => 3,
            RcMode::Mode4 => 4,
        }
    }
}

/// Axis mapping entry for UE-style input settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AxisBinding {
    pub axis_name: String,
    pub key: String,
    pub scale: f32,
    pub dead_zone: f32,
    pub exponent: f32,
    pub invert: bool,
}

/// Action mapping entry for button bindings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionBinding {
    pub action_name: String,
    pub key: String,
}

/// Input profile rendered into the managed AC7 block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ac7InputProfile {
    pub name: String,
    pub rc_mode: RcMode,
    pub enable_joystick: bool,
    pub steam_input_disabled_hint: bool,
    pub axis_bindings: Vec<AxisBinding>,
    pub action_bindings: Vec<ActionBinding>,
}

impl Default for Ac7InputProfile {
    fn default() -> Self {
        Self {
            name: "FlightHub HOTAS Default".to_string(),
            rc_mode: RcMode::Mode2,
            enable_joystick: true,
            steam_input_disabled_hint: true,
            axis_bindings: vec![
                AxisBinding {
                    axis_name: "Pitch".to_string(),
                    key: "Joystick_Axis1".to_string(),
                    scale: -1.0,
                    dead_zone: 0.03,
                    exponent: 1.0,
                    invert: false,
                },
                AxisBinding {
                    axis_name: "Roll".to_string(),
                    key: "Joystick_Axis0".to_string(),
                    scale: 1.0,
                    dead_zone: 0.03,
                    exponent: 1.0,
                    invert: false,
                },
                AxisBinding {
                    axis_name: "Yaw".to_string(),
                    key: "Joystick_Axis2".to_string(),
                    scale: 1.0,
                    dead_zone: 0.05,
                    exponent: 1.0,
                    invert: false,
                },
                AxisBinding {
                    axis_name: "Throttle".to_string(),
                    key: "Joystick_Axis3".to_string(),
                    scale: 1.0,
                    dead_zone: 0.0,
                    exponent: 1.0,
                    invert: false,
                },
            ],
            action_bindings: vec![
                ActionBinding {
                    action_name: "FireGun".to_string(),
                    key: "Joystick_Button0".to_string(),
                },
                ActionBinding {
                    action_name: "FireMissile".to_string(),
                    key: "Joystick_Button1".to_string(),
                },
                ActionBinding {
                    action_name: "ChangeWeapon".to_string(),
                    key: "Joystick_Button2".to_string(),
                },
                ActionBinding {
                    action_name: "TargetSwitch".to_string(),
                    key: "Joystick_Button3".to_string(),
                },
            ],
        }
    }
}

/// Result for profile install operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallResult {
    pub input_ini_path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub bytes_written: usize,
}

/// AC7 input config errors.
#[derive(Debug, Error)]
pub enum Ac7InputError {
    #[error("profile validation failed: {0}")]
    InvalidProfile(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl Ac7InputProfile {
    /// Validate profile fields.
    pub fn validate(&self) -> Result<(), Ac7InputError> {
        if self.name.trim().is_empty() || self.name.contains('"') {
            return Err(Ac7InputError::InvalidProfile(
                "profile name cannot be empty or contain double quotes".to_string(),
            ));
        }

        for axis in &self.axis_bindings {
            if axis.axis_name.trim().is_empty()
                || axis.key.trim().is_empty()
                || axis.axis_name.contains('"')
                || axis.key.contains('"')
            {
                return Err(Ac7InputError::InvalidProfile(format!(
                    "axis binding has empty or invalid axis/key: {:?}",
                    axis
                )));
            }
            if !(-2.0..=2.0).contains(&axis.scale) {
                return Err(Ac7InputError::InvalidProfile(format!(
                    "axis scale out of range for {}",
                    axis.axis_name
                )));
            }
            if !(0.0..=1.0).contains(&axis.dead_zone) {
                return Err(Ac7InputError::InvalidProfile(format!(
                    "axis dead zone out of range for {}",
                    axis.axis_name
                )));
            }
            if !(0.1..=5.0).contains(&axis.exponent) {
                return Err(Ac7InputError::InvalidProfile(format!(
                    "axis exponent out of range for {}",
                    axis.axis_name
                )));
            }
        }

        for action in &self.action_bindings {
            if action.action_name.trim().is_empty()
                || action.key.trim().is_empty()
                || action.action_name.contains('"')
                || action.key.contains('"')
            {
                return Err(Ac7InputError::InvalidProfile(format!(
                    "action binding has empty or invalid action/key: {:?}",
                    action
                )));
            }
        }

        Ok(())
    }
}

/// Render a Flight Hub managed block for AC7 Input.ini.
pub fn render_managed_block(profile: &Ac7InputProfile) -> Result<String, Ac7InputError> {
    profile.validate()?;

    let mut output = String::new();
    output.push_str(MANAGED_BLOCK_BEGIN);
    output.push('\n');
    output.push_str("; Generated by Flight Hub. Edit profile JSON instead of this block.\n");
    output.push_str("[FlightHub.AC7]\n");
    output.push_str(&format!("ProfileName=\"{}\"\n", profile.name));
    output.push_str(&format!("RCMode={}\n", profile.rc_mode.as_index()));
    output.push_str(&format!(
        "EnableJoystick={}\n",
        bool_to_ini(profile.enable_joystick)
    ));
    output.push_str(&format!(
        "SteamInputMustBeDisabled={}\n\n",
        bool_to_ini(profile.steam_input_disabled_hint)
    ));

    output.push_str("[/Script/Engine.InputSettings]\n");
    for axis in &profile.axis_bindings {
        output.push_str(&format!(
            "+AxisConfig=(AxisKeyName=\"{}\",AxisProperties=(DeadZone={:.4},Sensitivity=1.0,Exponent={:.4},bInvert={}))\n",
            axis.key,
            axis.dead_zone,
            axis.exponent,
            bool_to_ini(axis.invert)
        ));
        output.push_str(&format!(
            "+AxisMappings=(AxisName=\"{}\",Scale={:.4},Key={})\n",
            axis.axis_name, axis.scale, axis.key
        ));
    }
    for action in &profile.action_bindings {
        output.push_str(&format!(
            "+ActionMappings=(ActionName=\"{}\",Key={})\n",
            action.action_name, action.key
        ));
    }
    output.push_str(MANAGED_BLOCK_END);
    output.push('\n');

    Ok(output)
}

/// Apply profile block to existing Input.ini content.
///
/// Existing content is preserved, old managed block is replaced, and
/// `EnableJoystick=True` is always enforced.
pub fn apply_profile_to_existing(
    existing_content: &str,
    profile: &Ac7InputProfile,
) -> Result<String, Ac7InputError> {
    let stripped = strip_managed_block(existing_content);
    let mut output = upsert_enable_joystick_flag(&stripped);

    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    output.push('\n');
    output.push_str(&render_managed_block(profile)?);
    Ok(output)
}

/// Install or update Flight Hub AC7 input profile in Input.ini.
pub fn install_profile(
    input_ini_path: impl AsRef<Path>,
    profile: &Ac7InputProfile,
    create_backup: bool,
) -> Result<InstallResult, Ac7InputError> {
    let input_ini_path = input_ini_path.as_ref();
    let existing = if input_ini_path.exists() {
        fs::read_to_string(input_ini_path)?
    } else {
        String::new()
    };

    let backup_path = if create_backup && input_ini_path.exists() {
        let backup = input_ini_path.with_extension("ini.flight-hub.bak");
        fs::copy(input_ini_path, &backup)?;
        Some(backup)
    } else {
        None
    };

    if let Some(parent) = input_ini_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let updated = apply_profile_to_existing(&existing, profile)?;
    fs::write(input_ini_path, updated.as_bytes())?;

    Ok(InstallResult {
        input_ini_path: input_ini_path.to_path_buf(),
        backup_path,
        bytes_written: updated.len(),
    })
}

/// Default AC7 save-games path.
pub fn ac7_save_games_dir() -> Option<PathBuf> {
    let base = dirs::data_local_dir()?;
    Some(
        base.join("BANDAI NAMCO Entertainment")
            .join("ACE COMBAT 7")
            .join("SaveGames"),
    )
}

/// Default AC7 Input.ini path.
pub fn ac7_input_ini_path() -> Option<PathBuf> {
    let base = dirs::data_local_dir()?;
    Some(
        base.join("BANDAI NAMCO Entertainment")
            .join("ACE COMBAT 7")
            .join("Config")
            .join("WindowsNoEditor")
            .join("Input.ini"),
    )
}

/// User-facing reminder for Steam input.
pub fn steam_input_hint() -> &'static str {
    "Disable Steam Input for ACE COMBAT 7 before using custom HOTAS mappings."
}

fn strip_managed_block(content: &str) -> String {
    let mut output = String::new();
    let mut in_managed_block = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == MANAGED_BLOCK_BEGIN {
            in_managed_block = true;
            continue;
        }
        if trimmed == MANAGED_BLOCK_END {
            in_managed_block = false;
            continue;
        }
        if !in_managed_block {
            output.push_str(line);
            output.push('\n');
        }
    }

    output.trim_end().to_string()
}

fn upsert_enable_joystick_flag(content: &str) -> String {
    let mut output = String::new();
    let mut found_enable = false;
    let mut in_joystick_section = false;

    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            // Track whether we are inside [Joystick] section.
            in_joystick_section = trimmed.trim_end() == "[Joystick]";
        }
        if trimmed.starts_with("EnableJoystick=") {
            output.push_str("EnableJoystick=True\n");
            found_enable = true;
        } else {
            output.push_str(line);
            output.push('\n');
            // If we just entered [Joystick] and haven't seen the key yet,
            // insert it immediately after the section header.
            if in_joystick_section && !found_enable && trimmed.trim_end() == "[Joystick]" {
                output.push_str("EnableJoystick=True\n");
                found_enable = true;
                in_joystick_section = false;
            }
        }
    }

    if !found_enable {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("[Joystick]\n");
        output.push_str("EnableJoystick=True\n");
    }

    output.trim_end().to_string()
}

const fn bool_to_ini(value: bool) -> &'static str {
    if value { "True" } else { "False" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tempfile::tempdir;

    #[test]
    fn renders_managed_block() {
        let block = render_managed_block(&Ac7InputProfile::default()).unwrap();
        assert!(block.contains(MANAGED_BLOCK_BEGIN));
        assert!(block.contains(MANAGED_BLOCK_END));
        assert!(block.contains("EnableJoystick=True"));
        assert!(block.contains("+AxisMappings=(AxisName=\"Pitch\""));
    }

    #[test]
    fn applies_profile_replaces_existing_managed_block() {
        let existing = format!(
            "[Joystick]\nEnableJoystick=False\n{}\nold\n{}\n",
            MANAGED_BLOCK_BEGIN, MANAGED_BLOCK_END
        );
        let merged = apply_profile_to_existing(&existing, &Ac7InputProfile::default()).unwrap();
        assert_eq!(merged.matches(MANAGED_BLOCK_BEGIN).count(), 1);
        assert!(merged.contains("EnableJoystick=True"));
        assert!(!merged.contains("\nold\n"));
    }

    #[test]
    fn installs_with_backup() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("Input.ini");
        fs::write(&path, "[Joystick]\nEnableJoystick=False\n").unwrap();

        let result = install_profile(&path, &Ac7InputProfile::default(), true).unwrap();
        assert!(result.bytes_written > 0);
        assert!(result.backup_path.is_some());
        assert!(result.backup_path.unwrap().exists());

        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.contains(MANAGED_BLOCK_BEGIN));
        assert!(updated.contains("EnableJoystick=True"));
    }

    #[test]
    fn validates_empty_profile_name() {
        let mut profile = Ac7InputProfile::default();
        profile.name = String::new();
        assert!(profile.validate().is_err());
    }

    #[test]
    fn validates_out_of_range_scale() {
        let mut profile = Ac7InputProfile::default();
        profile.axis_bindings[0].scale = 5.0; // > 2.0
        assert!(profile.validate().is_err());
    }

    #[test]
    fn validates_out_of_range_deadzone() {
        let mut profile = Ac7InputProfile::default();
        profile.axis_bindings[0].dead_zone = 1.5; // > 1.0
        assert!(profile.validate().is_err());
    }

    #[test]
    fn validates_out_of_range_exponent() {
        let mut profile = Ac7InputProfile::default();
        profile.axis_bindings[0].exponent = 0.01; // < 0.1
        assert!(profile.validate().is_err());
    }

    #[test]
    fn rc_mode_index_values() {
        assert_eq!(RcMode::Mode1.as_index(), 1);
        assert_eq!(RcMode::Mode2.as_index(), 2);
        assert_eq!(RcMode::Mode3.as_index(), 3);
        assert_eq!(RcMode::Mode4.as_index(), 4);
    }

    #[test]
    fn managed_block_is_idempotent() {
        let profile = Ac7InputProfile::default();
        let first = apply_profile_to_existing("", &profile).unwrap();
        let second = apply_profile_to_existing(&first, &profile).unwrap();
        // Should have exactly one managed block in the final result
        assert_eq!(second.matches(MANAGED_BLOCK_BEGIN).count(), 1);
        assert_eq!(second.matches(MANAGED_BLOCK_END).count(), 1);
    }

    #[test]
    fn renders_all_rc_modes() {
        for mode in [RcMode::Mode1, RcMode::Mode2, RcMode::Mode3, RcMode::Mode4] {
            let mut profile = Ac7InputProfile::default();
            profile.rc_mode = mode;
            let block = render_managed_block(&profile).unwrap();
            assert!(block.contains(&format!("RCMode={}", mode.as_index())));
        }
    }

    #[test]
    fn install_creates_dir_and_file() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("config").join("input");
        let path = subdir.join("Input.ini");
        let result = install_profile(&path, &Ac7InputProfile::default(), false).unwrap();
        assert!(result.input_ini_path.exists());
        assert!(result.backup_path.is_none());
        assert!(result.bytes_written > 0);
    }

    #[test]
    fn steam_input_hint_is_nonempty() {
        assert!(!steam_input_hint().is_empty());
    }

    proptest! {
        #[test]
        fn property_valid_scale_range_accepted(scale in -2.0f32..=2.0f32) {
            let mut profile = Ac7InputProfile::default();
            profile.axis_bindings[0].scale = scale;
            prop_assert!(profile.validate().is_ok());
        }

        #[test]
        fn property_valid_deadzone_range_accepted(dz in 0.0f32..=1.0f32) {
            let mut profile = Ac7InputProfile::default();
            profile.axis_bindings[0].dead_zone = dz;
            prop_assert!(profile.validate().is_ok());
        }

        #[test]
        fn property_valid_exponent_range_accepted(exp in 0.1f32..=5.0f32) {
            let mut profile = Ac7InputProfile::default();
            profile.axis_bindings[0].exponent = exp;
            prop_assert!(profile.validate().is_ok());
        }
    }
}
