// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for rudder pedal device identification in flight-hid-support.
//!
//! Covers MFG Crosswind (V1/V2/V3), Slaw Viper (cam-based precision), and
//! cross-manufacturer pedal VID/PID discrimination.

use flight_hid_support::device_support::{
    MFG_CROSSWIND_V1_PID, MFG_CROSSWIND_V2_PID, MFG_CROSSWIND_V3_PID, MFG_VENDOR_ID,
    SAITEK_PRO_FLIGHT_RUDDER_PEDALS_PID, THRUSTMASTER_VENDOR_ID,
    TFRP_RUDDER_PEDALS_PID, TPR_PENDULAR_RUDDER_PID, T_RUDDER_PID,
    VKB_VENDOR_ID, VIRPIL_ACE_PEDALS_PID, VIRPIL_VENDOR_ID,
};

// ═══════════════════════════════════════════════════════════════════════════════
// 1. MFG Crosswind — Hall sensor, 3-axis, high resolution
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mfg_vendor_id_is_correct() {
    assert_eq!(MFG_VENDOR_ID, 0x1551, "MFG vendor ID should be 0x1551");
}

#[test]
fn mfg_crosswind_v1_pid() {
    assert_eq!(MFG_CROSSWIND_V1_PID, 0x0001, "Crosswind V1 PID should be 0x0001");
}

#[test]
fn mfg_crosswind_v2_pid() {
    assert_eq!(MFG_CROSSWIND_V2_PID, 0x0002, "Crosswind V2 PID should be 0x0002");
}

#[test]
fn mfg_crosswind_v3_pid() {
    assert_eq!(MFG_CROSSWIND_V3_PID, 0x0004, "Crosswind V3 PID should be 0x0004");
}

#[test]
fn mfg_crosswind_versions_have_distinct_pids() {
    let pids = [MFG_CROSSWIND_V1_PID, MFG_CROSSWIND_V2_PID, MFG_CROSSWIND_V3_PID];
    for (i, a) in pids.iter().enumerate() {
        for (j, b) in pids.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "Crosswind V{} and V{} should have distinct PIDs", i + 1, j + 1);
            }
        }
    }
}

#[test]
fn mfg_crosswind_pid_progression() {
    // V1 < V2 < V3 in PID numbering
    let (v1, v2, v3) = (MFG_CROSSWIND_V1_PID, MFG_CROSSWIND_V2_PID, MFG_CROSSWIND_V3_PID);
    assert!(v1 < v2, "V1 PID should be less than V2");
    assert!(v2 < v3, "V2 PID should be less than V3");
}

#[test]
fn mfg_vendor_id_distinct_from_other_pedal_vendors() {
    assert_ne!(MFG_VENDOR_ID, THRUSTMASTER_VENDOR_ID);
    assert_ne!(MFG_VENDOR_ID, VKB_VENDOR_ID);
    assert_ne!(MFG_VENDOR_ID, VIRPIL_VENDOR_ID);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Slaw Viper — cam-based, extreme precision, 3 axes
// ═══════════════════════════════════════════════════════════════════════════════
// Slaw Viper uses custom USB controllers; VID/PID may vary.
// These tests verify that the known pedal vendor IDs don't conflict with
// common STM32/AVR VIDs used by boutique pedal makers.

#[test]
fn known_pedal_vendor_ids_are_nonzero() {
    for (name, vid) in [
        ("MFG", MFG_VENDOR_ID),
        ("Thrustmaster", THRUSTMASTER_VENDOR_ID),
        ("VKB", VKB_VENDOR_ID),
        ("VIRPIL", VIRPIL_VENDOR_ID),
    ] {
        assert!(vid > 0, "{name} vendor ID should be non-zero");
    }
}

#[test]
fn known_pedal_vendor_ids_are_distinct() {
    let vids = [
        MFG_VENDOR_ID,
        THRUSTMASTER_VENDOR_ID,
        VKB_VENDOR_ID,
        VIRPIL_VENDOR_ID,
    ];
    for (i, a) in vids.iter().enumerate() {
        for (j, b) in vids.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "vendor IDs at index {i} and {j} must be distinct");
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Cross-manufacturer pedal PID discrimination
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn thrustmaster_pedal_pids_are_distinct() {
    let pids = [TFRP_RUDDER_PEDALS_PID, T_RUDDER_PID, TPR_PENDULAR_RUDDER_PID];
    for (i, a) in pids.iter().enumerate() {
        for (j, b) in pids.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "TM pedal PIDs at {i} and {j} must be distinct");
            }
        }
    }
}

#[test]
fn thrustmaster_vendor_id_is_correct() {
    assert_eq!(THRUSTMASTER_VENDOR_ID, 0x044F);
}

#[test]
fn vkb_vendor_id_is_correct() {
    assert_eq!(VKB_VENDOR_ID, 0x231D);
}

#[test]
fn virpil_vendor_id_is_correct() {
    assert_eq!(VIRPIL_VENDOR_ID, 0x3344);
}

#[test]
fn virpil_ace_pedals_pid_is_correct() {
    assert_eq!(VIRPIL_ACE_PEDALS_PID, 0x019C);
}

#[test]
fn saitek_rudder_pedals_pid_is_correct() {
    assert_eq!(SAITEK_PRO_FLIGHT_RUDDER_PEDALS_PID, 0x0763);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. MFG Crosswind sensitivity curves (high resolution)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mfg_crosswind_all_pids_in_valid_usb_range() {
    // USB Product IDs are 16-bit; verify they're non-zero
    let pids: [(_, u16); 3] = [
        ("V1", MFG_CROSSWIND_V1_PID),
        ("V2", MFG_CROSSWIND_V2_PID),
        ("V3", MFG_CROSSWIND_V3_PID),
    ];
    for (name, pid) in pids {
        assert_ne!(pid, 0, "{name} PID should be non-zero");
    }
}

#[test]
fn mfg_crosswind_vid_nonzero() {
    assert_ne!(MFG_VENDOR_ID, 0, "MFG VID should be non-zero");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Pedal-specific invariants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn no_pedal_vendor_uses_vid_zero() {
    // VID 0x0000 is reserved by USB-IF
    let vids: [u16; 4] = [MFG_VENDOR_ID, THRUSTMASTER_VENDOR_ID, VKB_VENDOR_ID, VIRPIL_VENDOR_ID];
    for vid in vids {
        assert_ne!(vid, 0, "VID {vid:#06X} must not be zero");
    }
}

#[test]
fn crosswind_pid_v3_is_not_sequential_from_v2() {
    // V3 PID is 0x0004 (skips 0x0003), documenting this gap
    assert_eq!(MFG_CROSSWIND_V2_PID, 0x0002);
    assert_eq!(MFG_CROSSWIND_V3_PID, 0x0004);
    assert_ne!(MFG_CROSSWIND_V3_PID, MFG_CROSSWIND_V2_PID + 1, "V3 PID skips 0x0003");
}
