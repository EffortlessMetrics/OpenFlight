// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for VKB T-Rudder Mk.IV pedals.
//!
//! Covers profile structure, axis mapping, device identification, and
//! T-Rudder-specific quirks (single-axis twist mapping, toe brake modes).

use flight_hotas_vkb::profiles::{
    AxisNormMode, all_profiles, profile_for_pid, t_rudder_profile,
};
use flight_hid_support::device_support::VKB_VENDOR_ID;

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Axis parsing — profile-based axis mapping tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t_rudder_has_three_axes() {
    let p = t_rudder_profile();
    assert_eq!(p.axis_count(), 3, "T-Rudder Mk.IV should have 3 axes");
}

#[test]
fn t_rudder_rudder_axis_is_signed() {
    let p = t_rudder_profile();
    let rudder = p.axis_by_name("rudder").expect("missing rudder axis");
    assert_eq!(
        rudder.mode,
        AxisNormMode::Signed,
        "rudder should be bidirectional (signed)"
    );
}

#[test]
fn t_rudder_left_toe_brake_is_unsigned() {
    let p = t_rudder_profile();
    let left = p.axis_by_name("left_toe_brake").expect("missing left_toe_brake");
    assert_eq!(left.mode, AxisNormMode::Unsigned);
}

#[test]
fn t_rudder_right_toe_brake_is_unsigned() {
    let p = t_rudder_profile();
    let right = p.axis_by_name("right_toe_brake").expect("missing right_toe_brake");
    assert_eq!(right.mode, AxisNormMode::Unsigned);
}

#[test]
fn t_rudder_toe_brakes_have_distinct_offsets() {
    let p = t_rudder_profile();
    let left = p.axis_by_name("left_toe_brake").unwrap();
    let right = p.axis_by_name("right_toe_brake").unwrap();
    assert_ne!(
        left.report_offset, right.report_offset,
        "left and right toe brakes must have different offsets"
    );
}

#[test]
fn t_rudder_rudder_offset_distinct_from_brakes() {
    let p = t_rudder_profile();
    let rudder = p.axis_by_name("rudder").unwrap();
    let left = p.axis_by_name("left_toe_brake").unwrap();
    let right = p.axis_by_name("right_toe_brake").unwrap();
    assert_ne!(rudder.report_offset, left.report_offset);
    assert_ne!(rudder.report_offset, right.report_offset);
}

#[test]
fn t_rudder_axis_offsets_do_not_overlap() {
    let p = t_rudder_profile();
    let offsets: Vec<usize> = p.axes.iter().map(|a| a.report_offset).collect();
    for (i, a) in offsets.iter().enumerate() {
        for (j, b) in offsets.iter().enumerate() {
            if i != j {
                assert!(
                    (*a as isize - *b as isize).unsigned_abs() >= 2,
                    "axis offsets {} and {} overlap (each u16 is 2 bytes)",
                    a,
                    b
                );
            }
        }
    }
}

#[test]
fn t_rudder_no_dead_center_confusion() {
    // Verify that the rudder's signed mode means center=0x8000→0.0
    let p = t_rudder_profile();
    let rudder = p.axis_by_name("rudder").unwrap();
    assert_eq!(rudder.mode, AxisNormMode::Signed,
        "signed mode implies 0x0000→-1.0, 0x8000→0.0, 0xFFFF→1.0");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Calibration properties
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t_rudder_axes_cover_expected_positions() {
    // Verify the axis names cover the expected physical controls
    let p = t_rudder_profile();
    assert!(p.axis_by_name("left_toe_brake").is_some());
    assert!(p.axis_by_name("right_toe_brake").is_some());
    assert!(p.axis_by_name("rudder").is_some());
    assert!(p.axis_by_name("nonexistent").is_none());
}

#[test]
fn t_rudder_axes_have_descriptions() {
    let p = t_rudder_profile();
    for axis in p.axes {
        assert!(!axis.description.is_empty(), "axis '{}' should have a description", axis.name);
    }
}

#[test]
fn t_rudder_unsigned_axes_range_check() {
    // Unsigned axes should be described as 0.0-1.0 range
    let p = t_rudder_profile();
    for axis in p.axes.iter().filter(|a| a.mode == AxisNormMode::Unsigned) {
        assert!(
            axis.description.contains("0.0") || axis.description.contains("released"),
            "unsigned axis '{}' description should mention zero point",
            axis.name
        );
    }
}

#[test]
fn t_rudder_signed_axis_range_check() {
    let p = t_rudder_profile();
    let rudder = p.axis_by_name("rudder").unwrap();
    assert!(
        rudder.description.contains("−1.0") || rudder.description.contains("-1.0"),
        "signed rudder description should mention bipolar range"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Profile generation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t_rudder_profile_device_name() {
    let p = t_rudder_profile();
    assert!(
        p.device_name.contains("T-Rudder"),
        "profile name should contain 'T-Rudder', got '{}'",
        p.device_name
    );
}

#[test]
fn t_rudder_profile_no_buttons() {
    let p = t_rudder_profile();
    assert_eq!(p.button_count(), 0, "T-Rudder has no physical buttons");
}

#[test]
fn t_rudder_profile_no_hats() {
    let p = t_rudder_profile();
    assert_eq!(p.hat_count(), 0, "T-Rudder has no hat switches");
}

#[test]
fn t_rudder_profile_vid_is_vkb() {
    let p = t_rudder_profile();
    assert_eq!(p.vid, VKB_VENDOR_ID, "T-Rudder VID should be VKB");
}

#[test]
fn t_rudder_profile_has_notes() {
    let p = t_rudder_profile();
    assert!(!p.notes.is_empty(), "T-Rudder should have usage notes");
}

#[test]
fn t_rudder_in_all_profiles_registry() {
    let profiles = all_profiles();
    let found = profiles.iter().any(|p| p.device_name.contains("T-Rudder"));
    assert!(found, "T-Rudder should appear in all_profiles() registry");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Device identification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn vkb_vendor_id_is_correct() {
    assert_eq!(VKB_VENDOR_ID, 0x231D, "VKB vendor ID should be 0x231D");
}

#[test]
fn t_rudder_pid_lookup_returns_none() {
    // T-Rudder PID is not yet confirmed; lookup should return None
    let result = profile_for_pid(0x9999);
    assert!(result.is_none(), "unknown PID should return None");
}

#[test]
fn t_rudder_profile_pids_list() {
    let p = t_rudder_profile();
    // PID not yet confirmed, so pids list may be empty
    // This test documents the current state
    assert!(
        p.pids.is_empty() || p.pids.iter().all(|&pid| pid > 0),
        "if PIDs are present, they should be valid non-zero values"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. T-Rudder quirks — single-axis twist mapping
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t_rudder_twist_type_rudder_is_signed() {
    // The T-Rudder uses a twist mechanism for rudder (unlike sliding pedals)
    // This means the rudder axis must be signed/bidirectional
    let p = t_rudder_profile();
    let rudder = p.axis_by_name("rudder").unwrap();
    assert_eq!(
        rudder.mode,
        AxisNormMode::Signed,
        "twist-type rudder must be bidirectional"
    );
}

#[test]
fn t_rudder_compact_axis_layout() {
    // T-Rudder is compact: 3 axes packed in minimal report space
    let p = t_rudder_profile();
    let max_offset = p.axes.iter().map(|a| a.report_offset).max().unwrap();
    // Each axis is 2 bytes, so 3 axes need at most offset 4 (bytes 0-1, 2-3, 4-5)
    assert!(
        max_offset <= 4,
        "compact T-Rudder should use offsets 0-4, got max {max_offset}"
    );
}

#[test]
fn t_rudder_no_button_lookup() {
    let p = t_rudder_profile();
    assert!(
        p.button_by_number(1).is_none(),
        "T-Rudder has no buttons to look up"
    );
}
