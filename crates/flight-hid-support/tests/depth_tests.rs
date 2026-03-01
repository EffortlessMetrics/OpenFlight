// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-hid-support: device enumeration, polling, report handling.

use flight_hid_support::HidDeviceInfo;
use flight_hid_support::device_support::*;
use flight_hid_support::ghost_filter::*;
use flight_hid_support::hid_descriptor::*;
use flight_hid_support::saitek_hotas::*;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Helper: build a HidDeviceInfo for testing
// ---------------------------------------------------------------------------

fn make_device(vid: u16, pid: u16) -> HidDeviceInfo {
    HidDeviceInfo {
        vendor_id: vid,
        product_id: pid,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: 0,
        usage: 0,
        report_descriptor: None,
    }
}

fn make_device_with_name(vid: u16, pid: u16, name: &str) -> HidDeviceInfo {
    HidDeviceInfo {
        vendor_id: vid,
        product_id: pid,
        serial_number: None,
        manufacturer: None,
        product_name: Some(name.to_string()),
        device_path: String::new(),
        usage_page: 0,
        usage: 0,
        report_descriptor: None,
    }
}

fn make_device_with_descriptor(vid: u16, pid: u16, descriptor: Vec<u8>) -> HidDeviceInfo {
    HidDeviceInfo {
        vendor_id: vid,
        product_id: pid,
        serial_number: None,
        manufacturer: None,
        product_name: None,
        device_path: String::new(),
        usage_page: 0,
        usage: 0,
        report_descriptor: Some(descriptor),
    }
}

// ---------------------------------------------------------------------------
// HID descriptor parsing: extract_usages
// ---------------------------------------------------------------------------

#[test]
fn descriptor_empty_returns_no_usages() {
    assert!(extract_usages(&[]).is_empty());
}

#[test]
fn descriptor_single_input_usage() {
    // Usage Page (Generic Desktop) = 0x05 0x01
    // Usage (Joystick)             = 0x09 0x04
    // Collection (Application)     = 0xA1 0x01
    //   Usage (X)                  = 0x09 0x30
    //   Input (Data,Var,Abs)       = 0x81 0x02
    // End Collection               = 0xC0
    let desc: &[u8] = &[
        0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, 0x09, 0x30, 0x81, 0x02, 0xC0,
    ];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].usage_page, 0x01);
    assert_eq!(usages[0].usage, 0x30); // X axis
}

#[test]
fn descriptor_multiple_axes() {
    // Usage Page(Generic Desktop), Usage(Joystick), Collection(Application),
    //   Usage(X), Usage(Y), Usage(Rz), Input
    // End Collection
    let desc: &[u8] = &[
        0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x35, 0x81, 0x02, 0xC0,
    ];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 3);
    assert_eq!(usages[0].usage, 0x30); // X
    assert_eq!(usages[1].usage, 0x31); // Y
    assert_eq!(usages[2].usage, 0x35); // Rz
}

#[test]
fn descriptor_usage_range_min_max() {
    // Usage Page (Button) = 0x05 0x09
    // Usage Minimum (1)   = 0x19 0x01
    // Usage Maximum (4)   = 0x29 0x04
    // Input               = 0x81 0x02
    let desc: &[u8] = &[0x05, 0x09, 0x19, 0x01, 0x29, 0x04, 0x81, 0x02];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 4);
    for (i, u) in usages.iter().enumerate() {
        assert_eq!(u.usage_page, 0x09);
        assert_eq!(u.usage, (i + 1) as u16);
    }
}

#[test]
fn descriptor_usage_range_reversed_min_max() {
    // Usage Minimum > Usage Maximum — parser should normalize
    let desc: &[u8] = &[0x05, 0x09, 0x19, 0x03, 0x29, 0x01, 0x81, 0x02];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 3);
    assert_eq!(usages[0].usage, 1);
    assert_eq!(usages[2].usage, 3);
}

#[test]
fn descriptor_long_item_skipped() {
    // 0xFE = long item prefix, length 2, tag 0x00, data [0xAA, 0xBB]
    // Then a normal Usage Page + Usage + Input
    let desc: &[u8] = &[
        0xFE, 0x02, 0x00, 0xAA, 0xBB, 0x05, 0x01, 0x09, 0x30, 0x81, 0x02,
    ];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].usage, 0x30);
}

#[test]
fn descriptor_truncated_gracefully_terminates() {
    // Incomplete item — only prefix, no data byte where size_code says 1
    let desc: &[u8] = &[0x05]; // Usage Page with size 1, but no data
    let usages = extract_usages(desc);
    assert!(usages.is_empty());
}

#[test]
fn descriptor_collection_clears_local_usages() {
    // Usage consumed by Collection should not appear in output;
    // only the Usage associated with an Input item should appear.
    let desc: &[u8] = &[
        0x05, 0x01, // Usage Page (Generic Desktop)
        0x09, 0x04, // Usage (Joystick) — consumed by Collection below
        0xA1, 0x01, // Collection (Application)
        0x09, 0x30, // Usage (X) — this is for the Input
        0x81, 0x02, // Input
        0xC0, // End Collection
    ];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 1);
    assert_eq!(usages[0].usage, 0x30);
}

#[test]
fn descriptor_output_item_also_extracts_usages() {
    // Output item (tag 0x09) should also trigger usage extraction
    let desc: &[u8] = &[
        0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, 0x09, 0x30, 0x91, 0x02, // Output
        0xC0,
    ];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 1);
}

#[test]
fn descriptor_feature_item_also_extracts_usages() {
    // Feature item (tag 0x0B) should also trigger usage extraction
    let desc: &[u8] = &[
        0x05, 0x01, 0x09, 0x04, 0xA1, 0x01, 0x09, 0x30, 0xB1, 0x02, // Feature
        0xC0,
    ];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 1);
}

#[test]
fn descriptor_multiple_usage_pages() {
    // Generic Desktop axes, then switch to Button page
    let desc: &[u8] = &[
        0x05, 0x01, // Usage Page (Generic Desktop)
        0x09, 0x04, // Usage (Joystick)
        0xA1, 0x01, // Collection
        0x09, 0x30, // Usage (X)
        0x81, 0x02, // Input
        0x05, 0x09, // Usage Page (Button)
        0x19, 0x01, // Usage Min (1)
        0x29, 0x02, // Usage Max (2)
        0x81, 0x02, // Input
        0xC0,
    ];
    let usages = extract_usages(desc);
    assert_eq!(usages.len(), 3);
    assert_eq!(usages[0].usage_page, 0x01); // Generic Desktop
    assert_eq!(usages[1].usage_page, 0x09); // Button
    assert_eq!(usages[2].usage_page, 0x09);
}

#[test]
fn descriptor_large_usage_range_capped() {
    // Usage range > 64 items should be silently capped
    let desc: &[u8] = &[0x05, 0x09, 0x19, 0x01, 0x29, 0xFF, 0x81, 0x02];
    let usages = extract_usages(desc);
    // Range is 0x01..=0xFF = 255 items which exceeds the 64-item cap
    assert!(usages.is_empty());
}

// ---------------------------------------------------------------------------
// Axis mode detection from descriptors
// ---------------------------------------------------------------------------

#[test]
fn axis_mode_merged_descriptor() {
    // Merged: X, Y, Rz, no sliders
    let desc = parse_hex_descriptor(
        "05 01 09 04 A1 01 09 30 09 31 09 35 15 81 25 7F 75 08 95 03 81 02 C0",
    );
    assert_eq!(axis_mode_from_descriptor(&desc), AxisMode::Merged);
}

#[test]
fn axis_mode_separate_descriptor() {
    // Separate: X, Y, Rz, 2 sliders
    let desc = parse_hex_descriptor(
        "05 01 09 04 A1 01 09 30 09 31 09 35 09 36 09 36 15 81 25 7F 75 08 95 05 81 02 C0",
    );
    assert_eq!(axis_mode_from_descriptor(&desc), AxisMode::Separate);
}

#[test]
fn axis_mode_unknown_missing_rz() {
    // Has X, Y but no Rz
    let desc: &[u8] = &[
        0x05, 0x01, 0xA1, 0x01, 0x09, 0x30, 0x09, 0x31, 0x81, 0x02, 0xC0,
    ];
    assert_eq!(axis_mode_from_descriptor(desc), AxisMode::Unknown);
}

#[test]
fn axis_mode_unknown_one_slider() {
    // X, Y, Rz, 1 slider — ambiguous
    let desc: &[u8] = &[
        0x05, 0x01, 0xA1, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x35, 0x09, 0x36, 0x81, 0x02, 0xC0,
    ];
    assert_eq!(axis_mode_from_descriptor(desc), AxisMode::Unknown);
}

#[test]
fn axis_mode_from_device_info_no_descriptor() {
    let dev = make_device(THRUSTMASTER_VENDOR_ID, TFLIGHT_HOTAS_4_PID);
    assert_eq!(axis_mode_from_device_info(&dev), AxisMode::Unknown);
}

#[test]
fn axis_mode_from_device_info_with_descriptor() {
    let desc = parse_hex_descriptor(
        "05 01 09 04 A1 01 09 30 09 31 09 35 15 81 25 7F 75 08 95 03 81 02 C0",
    );
    let dev = make_device_with_descriptor(THRUSTMASTER_VENDOR_ID, TFLIGHT_HOTAS_4_PID, desc);
    assert_eq!(axis_mode_from_device_info(&dev), AxisMode::Merged);
}

#[test]
fn axis_mode_as_str_values() {
    assert_eq!(AxisMode::Merged.as_str(), "merged");
    assert_eq!(AxisMode::Separate.as_str(), "separate");
    assert_eq!(AxisMode::Unknown.as_str(), "unknown");
}

// ---------------------------------------------------------------------------
// AxisUsageSummary
// ---------------------------------------------------------------------------

#[test]
fn axis_usage_summary_empty() {
    let summary = AxisUsageSummary::from_usages(&[]);
    assert!(!summary.has_x);
    assert!(!summary.has_y);
    assert!(!summary.has_rz);
    assert_eq!(summary.slider_like_count, 0);
}

#[test]
fn axis_usage_summary_full_axes() {
    let usages = [
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_X,
        },
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_Y,
        },
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_RZ,
        },
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_SLIDER,
        },
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_DIAL,
        },
    ];
    let summary = AxisUsageSummary::from_usages(&usages);
    assert!(summary.has_x);
    assert!(summary.has_y);
    assert!(summary.has_rz);
    assert_eq!(summary.slider_like_count, 2);
}

#[test]
fn axis_usage_summary_ignores_non_generic_desktop() {
    let usages = [HidUsage {
        usage_page: USAGE_PAGE_BUTTON,
        usage: USAGE_X,
    }];
    let summary = AxisUsageSummary::from_usages(&usages);
    assert!(!summary.has_x);
}

// ---------------------------------------------------------------------------
// T.Flight device identification
// ---------------------------------------------------------------------------

#[test]
fn tflight_hotas_one_by_pid() {
    let dev = make_device(THRUSTMASTER_VENDOR_ID, TFLIGHT_HOTAS_ONE_PID);
    assert_eq!(tflight_model(&dev), Some(TFlightModel::HotasOne));
    assert!(is_tflight_device(&dev));
}

#[test]
fn tflight_hotas_4_by_pid() {
    let dev = make_device(THRUSTMASTER_VENDOR_ID, TFLIGHT_HOTAS_4_PID);
    assert_eq!(tflight_model(&dev), Some(TFlightModel::Hotas4));
}

#[test]
fn tflight_hotas_4_legacy_pid() {
    let dev = make_device(THRUSTMASTER_VENDOR_ID, TFLIGHT_HOTAS_4_PID_LEGACY);
    assert_eq!(tflight_model(&dev), Some(TFlightModel::Hotas4));
    assert!(is_hotas4_legacy_pid(&dev));
}

#[test]
fn tflight_hotas_x_by_pid() {
    let dev = make_device(THRUSTMASTER_VENDOR_ID, TFLIGHT_HOTAS_X_PID);
    assert_eq!(tflight_model(&dev), Some(TFlightModel::HotasX));
}

#[test]
fn tflight_detected_by_product_name_fallback() {
    let dev = make_device_with_name(THRUSTMASTER_VENDOR_ID, 0xFFFF, "T.Flight Hotas One");
    assert_eq!(tflight_model(&dev), Some(TFlightModel::HotasOne));
}

#[test]
fn tflight_name_fallback_hotas4() {
    let dev = make_device_with_name(THRUSTMASTER_VENDOR_ID, 0xFFFF, "T.Flight Hotas 4 PS4");
    assert_eq!(tflight_model(&dev), Some(TFlightModel::Hotas4));
}

#[test]
fn tflight_name_fallback_hotasx() {
    let dev = make_device_with_name(THRUSTMASTER_VENDOR_ID, 0xFFFF, "Something HOTAS X edition");
    assert_eq!(tflight_model(&dev), Some(TFlightModel::HotasX));
}

#[test]
fn tflight_wrong_vendor_not_detected() {
    let dev = make_device(0x1234, TFLIGHT_HOTAS_4_PID);
    assert!(tflight_model(&dev).is_none());
    assert!(!is_tflight_device(&dev));
}

#[test]
fn tflight_model_names_nonempty() {
    assert!(!TFlightModel::HotasOne.name().is_empty());
    assert!(!TFlightModel::Hotas4.name().is_empty());
    assert!(!TFlightModel::HotasX.name().is_empty());
}

// ---------------------------------------------------------------------------
// Logitech device identification
// ---------------------------------------------------------------------------

#[test]
fn logitech_extreme_3d_pro_detection() {
    assert!(is_extreme_3d_pro(LOGITECH_VENDOR_ID, EXTREME_3D_PRO_PID));
    assert!(!is_extreme_3d_pro(LOGITECH_VENDOR_ID, 0xFFFF));
    assert!(!is_extreme_3d_pro(0x1234, EXTREME_3D_PRO_PID));
}

#[test]
fn logitech_g940_joystick_detection() {
    assert!(is_g940_joystick(LOGITECH_VENDOR_ID, G940_FLIGHT_SYSTEM_PID));
    assert!(!is_g940_joystick(LOGITECH_VENDOR_ID, 0xFFFF));
}

#[test]
fn logitech_g940_throttle_detection() {
    assert!(is_g940_throttle(LOGITECH_VENDOR_ID, G940_THROTTLE_PID));
    assert!(!is_g940_throttle(0x1234, G940_THROTTLE_PID));
}

#[test]
fn logitech_g_flight_yoke_detection() {
    assert!(is_g_flight_yoke(LOGITECH_VENDOR_ID, G_FLIGHT_YOKE_PID));
    assert!(!is_g_flight_yoke(0x1234, G_FLIGHT_YOKE_PID));
}

#[test]
fn logitech_g_flight_throttle_quadrant_detection() {
    assert!(is_g_flight_throttle_quadrant(
        LOGITECH_VENDOR_ID,
        G_FLIGHT_THROTTLE_QUADRANT_PID
    ));
    assert!(!is_g_flight_throttle_quadrant(
        0x1234,
        G_FLIGHT_THROTTLE_QUADRANT_PID
    ));
}

// ---------------------------------------------------------------------------
// Microsoft SideWinder detection
// ---------------------------------------------------------------------------

#[test]
fn sidewinder_ffb_pro_detection() {
    assert!(is_sidewinder_device(
        MICROSOFT_VENDOR_ID,
        SIDEWINDER_FFB_PRO_PID
    ));
    assert_eq!(
        sidewinder_model(SIDEWINDER_FFB_PRO_PID),
        Some(SidewinderModel::FfbPro)
    );
    assert!(SidewinderModel::FfbPro.has_ffb());
}

#[test]
fn sidewinder_ffb2_detection() {
    assert!(is_sidewinder_device(
        MICROSOFT_VENDOR_ID,
        SIDEWINDER_FFB2_PID
    ));
    assert_eq!(
        sidewinder_model(SIDEWINDER_FFB2_PID),
        Some(SidewinderModel::Ffb2)
    );
    assert!(SidewinderModel::Ffb2.has_ffb());
}

#[test]
fn sidewinder_precision2_no_ffb() {
    assert_eq!(
        sidewinder_model(SIDEWINDER_PRECISION_2_PID),
        Some(SidewinderModel::Precision2)
    );
    assert!(!SidewinderModel::Precision2.has_ffb());
}

#[test]
fn sidewinder_unknown_pid() {
    assert!(sidewinder_model(0xFFFF).is_none());
    assert!(!is_sidewinder_device(MICROSOFT_VENDOR_ID, 0xFFFF));
}

#[test]
fn sidewinder_model_names_nonempty() {
    assert!(!SidewinderModel::FfbPro.name().is_empty());
    assert!(!SidewinderModel::Ffb2.name().is_empty());
    assert!(!SidewinderModel::Precision2.name().is_empty());
}

// ---------------------------------------------------------------------------
// Thrustmaster family detection (Warthog, T16000M, Cougar, TCA)
// ---------------------------------------------------------------------------

#[test]
fn warthog_joystick_and_throttle() {
    assert!(is_warthog_device(
        THRUSTMASTER_VENDOR_ID,
        WARTHOG_JOYSTICK_PID
    ));
    assert!(is_warthog_device(
        THRUSTMASTER_VENDOR_ID,
        WARTHOG_THROTTLE_PID
    ));
    assert_eq!(
        warthog_model(WARTHOG_JOYSTICK_PID),
        Some(WarthogModel::Joystick)
    );
    assert_eq!(
        warthog_model(WARTHOG_THROTTLE_PID),
        Some(WarthogModel::Throttle)
    );
    assert!(warthog_model(0xFFFF).is_none());
}

#[test]
fn warthog_model_names() {
    assert!(!WarthogModel::Joystick.name().is_empty());
    assert!(!WarthogModel::Throttle.name().is_empty());
}

#[test]
fn t16000m_device_detection() {
    assert!(is_t16000m_device(
        THRUSTMASTER_VENDOR_ID,
        T16000M_JOYSTICK_PID
    ));
    assert!(is_t16000m_device(THRUSTMASTER_VENDOR_ID, TWCS_THROTTLE_PID));
    assert!(!is_t16000m_device(THRUSTMASTER_VENDOR_ID, 0xFFFF));
    assert_eq!(
        t16000m_model(T16000M_JOYSTICK_PID),
        Some(T16000mModel::Joystick)
    );
    assert_eq!(
        t16000m_model(TWCS_THROTTLE_PID),
        Some(T16000mModel::TwcsThrottle)
    );
}

#[test]
fn cougar_hotas_detection() {
    assert!(is_cougar_hotas_device(
        THRUSTMASTER_VENDOR_ID,
        COUGAR_HOTAS_STICK_PID
    ));
    assert!(!is_cougar_hotas_device(0x1234, COUGAR_HOTAS_STICK_PID));
    assert_eq!(
        cougar_hotas_model(COUGAR_HOTAS_STICK_PID),
        Some(CougarHotasModel::Stick)
    );
    assert!(cougar_hotas_model(0xFFFF).is_none());
}

#[test]
fn thrustmaster_usb_joystick_detection() {
    assert!(is_thrustmaster_usb_joystick(
        THRUSTMASTER_VENDOR_ID,
        THRUSTMASTER_USB_JOYSTICK_PID
    ));
    assert!(!is_thrustmaster_usb_joystick(
        0x1234,
        THRUSTMASTER_USB_JOYSTICK_PID
    ));
}

#[test]
fn tca_airbus_all_models() {
    let models = [
        (
            TCA_SIDESTICK_AIRBUS_PILOT_PID,
            TcaAirbusModel::SidestickPilot,
        ),
        (
            TCA_SIDESTICK_AIRBUS_COPILOT_PID,
            TcaAirbusModel::SidestickCopilot,
        ),
        (TCA_QUADRANT_AIRBUS_ENG12_PID, TcaAirbusModel::QuadrantEng12),
        (TCA_QUADRANT_AIRBUS_ENG34_PID, TcaAirbusModel::QuadrantEng34),
    ];
    for (pid, expected) in models {
        assert!(is_tca_airbus_device(THRUSTMASTER_VENDOR_ID, pid));
        assert_eq!(tca_airbus_model(pid), Some(expected));
        assert!(!expected.name().is_empty());
    }
    assert!(!is_tca_airbus_device(THRUSTMASTER_VENDOR_ID, 0xFFFF));
    assert!(tca_airbus_model(0xFFFF).is_none());
}

#[test]
fn tca_boeing_all_models() {
    let models = [
        (TCA_YOKE_BOEING_PID, TcaBoeingModel::YokeBoeing),
        (
            TCA_QUADRANT_BOEING_ENG12_PID,
            TcaBoeingModel::QuadrantBoeing12,
        ),
        (
            TCA_QUADRANT_BOEING_ENG34_PID,
            TcaBoeingModel::QuadrantBoeing34,
        ),
    ];
    for (pid, expected) in models {
        assert!(is_tca_boeing_device(THRUSTMASTER_VENDOR_ID, pid));
        assert_eq!(tca_boeing_model(pid), Some(expected));
        assert!(!expected.name().is_empty());
    }
    assert!(!is_tca_boeing_device(THRUSTMASTER_VENDOR_ID, 0xFFFF));
}

// ---------------------------------------------------------------------------
// VIRPIL device detection
// ---------------------------------------------------------------------------

#[test]
fn virpil_all_models_detected() {
    let models = [
        (VIRPIL_CM2_THROTTLE_PID, VirpilModel::Cm2Throttle),
        (VIRPIL_CM2_STICK_PID, VirpilModel::Cm2Stick),
        (VIRPIL_CM3_THROTTLE_PID, VirpilModel::Cm3Throttle),
        (VIRPIL_MONGOOST_STICK_PID, VirpilModel::MongoostStick),
        (VIRPIL_PANEL1_PID, VirpilModel::ControlPanel1),
        (VIRPIL_PANEL2_PID, VirpilModel::ControlPanel2),
        (VIRPIL_SHARK_PANEL_PID, VirpilModel::SharkPanel),
        (
            VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
            VirpilModel::ConstellationAlphaLeft,
        ),
        (
            VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID,
            VirpilModel::ConstellationAlphaPrimeLeft,
        ),
        (
            VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID,
            VirpilModel::ConstellationAlphaPrimeRight,
        ),
        (VIRPIL_WARBRD_PID, VirpilModel::WarBrd),
        (VIRPIL_WARBRD_D_PID, VirpilModel::WarBrdD),
        (VIRPIL_ACE_TORQ_PID, VirpilModel::AceTorq),
        (VIRPIL_ACE_PEDALS_PID, VirpilModel::AcePedals),
        (VIRPIL_ROTOR_TCS_PLUS_PID, VirpilModel::RotorTcsPlus),
    ];
    for (pid, expected) in models {
        assert!(is_virpil_device(VIRPIL_VENDOR_ID, pid));
        assert_eq!(virpil_model(pid), Some(expected));
        assert!(!expected.name().is_empty());
    }
    assert!(!is_virpil_device(VIRPIL_VENDOR_ID, 0xFFFF));
    assert!(virpil_model(0xFFFF).is_none());
}

// ---------------------------------------------------------------------------
// CH Products detection
// ---------------------------------------------------------------------------

#[test]
fn ch_all_models_detected() {
    let models = [
        (CH_PRO_THROTTLE_PID, ChModel::ProThrottle),
        (CH_PRO_PEDALS_PID, ChModel::ProPedals),
        (CH_FIGHTERSTICK_PID, ChModel::Fighterstick),
        (CH_COMBAT_STICK_PID, ChModel::CombatStick),
        (CH_ECLIPSE_YOKE_PID, ChModel::EclipseYoke),
        (CH_FLIGHT_YOKE_PID, ChModel::FlightYoke),
    ];
    for (pid, expected) in models {
        assert!(is_ch_device(CH_VENDOR_ID, pid));
        assert_eq!(ch_model(pid), Some(expected));
        assert!(!expected.name().is_empty());
    }
    assert!(!is_ch_device(CH_VENDOR_ID, 0xFFFF));
}

// ---------------------------------------------------------------------------
// VPforce detection
// ---------------------------------------------------------------------------

#[test]
fn vpforce_rhino_detection() {
    assert!(is_vpforce_device(VPFORCE_VENDOR_ID, VPFORCE_RHINO_PID_V2));
    assert!(is_vpforce_device(VPFORCE_VENDOR_ID, VPFORCE_RHINO_PID_V3));
    assert!(!is_vpforce_device(VPFORCE_VENDOR_ID, 0xFFFF));
    assert_eq!(
        vpforce_model(VPFORCE_RHINO_PID_V2),
        Some(VpforceModel::RhinoV2)
    );
    assert_eq!(
        vpforce_model(VPFORCE_RHINO_PID_V3),
        Some(VpforceModel::RhinoV3)
    );
    assert!(!VpforceModel::RhinoV2.name().is_empty());
    assert!(!VpforceModel::RhinoV3.name().is_empty());
}

// ---------------------------------------------------------------------------
// Moza detection
// ---------------------------------------------------------------------------

#[test]
fn moza_detection() {
    assert!(is_moza_device(MOZA_VENDOR_ID, MOZA_AB9_PID));
    assert!(is_moza_device(MOZA_VENDOR_ID, MOZA_R3_PID));
    assert!(!is_moza_device(MOZA_VENDOR_ID, 0xFFFF));
    assert_eq!(moza_model(MOZA_AB9_PID), Some(MozaModel::Ab9));
    assert_eq!(moza_model(MOZA_R3_PID), Some(MozaModel::R3));
    assert!(!MozaModel::Ab9.name().is_empty());
}

// ---------------------------------------------------------------------------
// Brunner detection
// ---------------------------------------------------------------------------

#[test]
fn brunner_all_models() {
    let models = [
        (BRUNNER_CLS_E_YOKE_PID, BrunnerModel::ClsE),
        (BRUNNER_CLS_E_JOYSTICK_PID, BrunnerModel::ClsEJoystick),
        (BRUNNER_CLS_E_NG_YOKE_PID, BrunnerModel::ClsENgYoke),
        (BRUNNER_CLS_E_RUDDER_PID, BrunnerModel::ClsERudder),
    ];
    for (pid, expected) in models {
        assert!(is_brunner_device(BRUNNER_VENDOR_ID, pid));
        assert_eq!(brunner_model(pid), Some(expected));
        assert!(!expected.name().is_empty());
    }
    assert!(!is_brunner_device(BRUNNER_VENDOR_ID, 0xFFFF));
}

// ---------------------------------------------------------------------------
// Saitek HOTAS (re-exported via lib.rs)
// ---------------------------------------------------------------------------

#[test]
fn saitek_x52_identification_and_properties() {
    let t = SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X52_PID).unwrap();
    assert_eq!(t, SaitekHotasType::X52);
    assert!(t.is_unified_topology());
    assert!(!t.is_split_topology());
    assert!(t.is_stick());
    assert!(!t.is_throttle());
    assert!(t.has_leds());
    assert!(!t.has_mfd());
    assert!(!t.has_rgb());
    assert_eq!(t.family(), SaitekHotasFamily::X52);
    assert!(!t.name().is_empty());
    assert!(!t.short_name().is_empty());
}

#[test]
fn saitek_x52_pro_has_mfd_and_leds() {
    let t = SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X52_PRO_PID).unwrap();
    assert_eq!(t, SaitekHotasType::X52Pro);
    assert!(t.has_mfd());
    assert!(t.has_leds());
    assert!(!t.has_rgb());
    assert_eq!(t.family(), SaitekHotasFamily::X52);
}

#[test]
fn saitek_x65f_unified_topology() {
    let t = SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X65F_PID).unwrap();
    assert_eq!(t, SaitekHotasType::X65F);
    assert!(t.is_unified_topology());
    assert!(t.has_leds());
    assert_eq!(t.family(), SaitekHotasFamily::X65);
}

#[test]
fn saitek_x55_dual_vid_detection() {
    // Saitek VID
    assert_eq!(
        SaitekHotasType::from_vid_pid(SAITEK_VENDOR_ID, X55_STICK_PID),
        Some(SaitekHotasType::X55Stick)
    );
    // Mad Catz VID (some units shipped this way)
    assert_eq!(
        SaitekHotasType::from_vid_pid(MAD_CATZ_VENDOR_ID, X55_THROTTLE_PID),
        Some(SaitekHotasType::X55Throttle)
    );
}

#[test]
fn saitek_x56_logitech_throttle_intentionally_not_matched() {
    // The Logitech throttle PID (0xC22A) collides with G110 keyboard
    assert!(SaitekHotasType::from_vid_pid(LOGITECH_VENDOR_ID, 0xC22A).is_none());
}

#[test]
fn saitek_x56_madcatz_both_components() {
    let stick = SaitekHotasType::from_vid_pid(MAD_CATZ_VENDOR_ID, X56_MADCATZ_STICK_PID).unwrap();
    let throttle =
        SaitekHotasType::from_vid_pid(MAD_CATZ_VENDOR_ID, X56_MADCATZ_THROTTLE_PID).unwrap();
    assert_eq!(stick, SaitekHotasType::X56Stick);
    assert_eq!(throttle, SaitekHotasType::X56Throttle);
    assert!(stick.is_split_topology());
    assert!(stick.has_rgb());
    assert!(throttle.has_rgb());
    assert_eq!(stick.family(), SaitekHotasFamily::X56);
}

#[test]
fn is_saitek_hotas_boundary() {
    assert!(is_saitek_hotas(SAITEK_VENDOR_ID, X52_PID));
    assert!(!is_saitek_hotas(0x0000, 0x0000));
    assert!(!is_saitek_hotas(SAITEK_VENDOR_ID, 0xFFFF));
}

#[test]
fn saitek_family_names_nonempty() {
    assert!(!SaitekHotasFamily::X52.name().is_empty());
    assert!(!SaitekHotasFamily::X55.name().is_empty());
    assert!(!SaitekHotasFamily::X56.name().is_empty());
    assert!(!SaitekHotasFamily::X65.name().is_empty());
}

// ---------------------------------------------------------------------------
// VKB detection
// ---------------------------------------------------------------------------

#[test]
fn vkb_gladiator_variant_detection() {
    let right = make_device(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
    let left = make_device(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_LEFT_PID);
    assert_eq!(
        vkb_gladiator_variant(&right),
        Some(VkbGladiatorVariant::NxtEvoRight)
    );
    assert_eq!(
        vkb_gladiator_variant(&left),
        Some(VkbGladiatorVariant::NxtEvoLeft)
    );
    assert!(is_vkb_gladiator_device(&right));
    assert!(is_vkb_gladiator_device(&left));
    assert!(!VkbGladiatorVariant::NxtEvoRight.name().is_empty());
}

#[test]
fn vkb_gladiator_control_map_has_axes() {
    let map = vkb_gladiator_control_map(VkbGladiatorVariant::NxtEvoRight);
    assert!(!map.axes.is_empty());
    assert!(!map.schema.is_empty());
    assert!(!map.notes.is_empty());
    // Left variant also has axes
    let map_l = vkb_gladiator_control_map(VkbGladiatorVariant::NxtEvoLeft);
    assert!(!map_l.axes.is_empty());
}

#[test]
fn vkb_stecs_variant_detection() {
    let dev = make_device(VKB_VENDOR_ID, VKB_STECS_LEFT_SPACE_MINI_PID);
    assert!(is_vkb_stecs_device(&dev));
    let variant = vkb_stecs_variant(&dev).unwrap();
    assert!(!variant.name().is_empty());
}

// ---------------------------------------------------------------------------
// Descriptor discovery
// ---------------------------------------------------------------------------

#[test]
fn descriptor_discovery_from_usages_basic() {
    let usages = vec![
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_X,
        },
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_Y,
        },
        HidUsage {
            usage_page: USAGE_PAGE_GENERIC_DESKTOP,
            usage: USAGE_HAT_SWITCH,
        },
        HidUsage {
            usage_page: USAGE_PAGE_BUTTON,
            usage: 1,
        },
        HidUsage {
            usage_page: USAGE_PAGE_BUTTON,
            usage: 2,
        },
    ];
    let disc = descriptor_discovery_from_usages(&usages);
    assert_eq!(disc.counts.axes, 2);
    assert_eq!(disc.counts.hats, 1);
    assert_eq!(disc.counts.buttons, 2);
    assert_eq!(disc.schema, "flight.hid-discovery/1");
    assert!(!disc.notes.is_empty());
}

#[test]
fn descriptor_discovery_axis_labels() {
    let usages = vec![HidUsage {
        usage_page: USAGE_PAGE_GENERIC_DESKTOP,
        usage: USAGE_SLIDER,
    }];
    let disc = descriptor_discovery_from_usages(&usages);
    assert_eq!(disc.axes.len(), 1);
    assert_eq!(disc.axes[0].label, "Slider");
    assert_eq!(
        disc.axes[0].suggested_logical,
        Some("throttle_candidate".to_string())
    );
}

#[test]
fn descriptor_discovery_from_device_info_adds_vkb_notes() {
    let desc = parse_hex_descriptor("05 01 09 04 A1 01 09 30 81 02 C0");
    let dev = make_device_with_descriptor(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID, desc);
    let disc = descriptor_discovery_from_device_info(&dev).unwrap();
    // Should contain VKB-specific and Gladiator-specific notes
    assert!(disc.notes.iter().any(|n| n.contains("VKBDevCfg")));
}

#[test]
fn descriptor_discovery_from_device_info_none_without_descriptor() {
    let dev = make_device(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
    assert!(descriptor_discovery_from_device_info(&dev).is_none());
}

// ---------------------------------------------------------------------------
// Default mappings / warnings / notes
// ---------------------------------------------------------------------------

#[test]
fn axis_mode_warning_only_for_merged() {
    assert!(axis_mode_warning(AxisMode::Merged).is_some());
    assert!(axis_mode_warning(AxisMode::Separate).is_none());
    assert!(axis_mode_warning(AxisMode::Unknown).is_none());
}

#[test]
fn default_mapping_note_only_for_unknown() {
    assert!(default_mapping_note(AxisMode::Unknown).is_some());
    assert!(default_mapping_note(AxisMode::Merged).is_none());
    assert!(default_mapping_note(AxisMode::Separate).is_none());
}

#[test]
fn driver_note_nonempty() {
    assert!(!driver_note().is_empty());
}

#[test]
fn pc_mode_notes_per_model() {
    assert!(!pc_mode_note(TFlightModel::Hotas4).is_empty());
    assert!(!pc_mode_note(TFlightModel::HotasOne).is_empty());
    assert!(!pc_mode_note(TFlightModel::HotasX).is_empty());
}

#[test]
fn tflight_default_mapping_separate_has_bindings() {
    let mapping = tflight_default_mapping(AxisMode::Separate);
    assert!(!mapping.bindings.is_empty());
    assert!(mapping.bindings.len() >= 4);
    let hint = mapping.as_hint_string();
    assert!(hint.contains("Roll"));
    assert!(hint.contains("Pitch"));
}

#[test]
fn tflight_default_mapping_merged_has_bindings() {
    let mapping = tflight_default_mapping(AxisMode::Merged);
    assert!(!mapping.bindings.is_empty());
}

// ---------------------------------------------------------------------------
// Display implementations
// ---------------------------------------------------------------------------

#[test]
fn axis_usage_display() {
    assert_eq!(format!("{}", AxisUsage::X), "X");
    assert_eq!(format!("{}", AxisUsage::Rz), "RZ");
    assert_eq!(format!("{}", AxisUsage::Slider0), "Slider0");
    assert_eq!(format!("{}", AxisUsage::RzCombined), "RZ (combined)");
}

#[test]
fn physical_control_display() {
    assert_eq!(format!("{}", PhysicalControl::Axis(AxisUsage::X)), "X");
    assert_eq!(format!("{}", PhysicalControl::Hat), "Hat");
}

#[test]
fn logical_control_display() {
    assert_eq!(format!("{}", LogicalControl::Roll), "Roll");
    assert_eq!(format!("{}", LogicalControl::Throttle), "Throttle");
}

// ---------------------------------------------------------------------------
// Ghost filter: ImpossibleStateDetector
// ---------------------------------------------------------------------------

#[test]
fn impossible_detector_no_masks_passes_everything() {
    let mut det = ImpossibleStateDetector::new(vec![]);
    assert_eq!(det.filter(0xFFFFFFFF), 0xFFFFFFFF);
}

#[test]
fn impossible_detector_single_bit_mask_ignored() {
    // Single-bit masks should be ignored (count_ones < 2)
    let mut det = ImpossibleStateDetector::new(vec![0b0001]);
    assert_eq!(det.filter(0b0001), 0b0001);
}

#[test]
fn impossible_detector_multi_mask_cascade() {
    let mut det = ImpossibleStateDetector::new(vec![0b0011, 0b1100]);
    // First, establish a valid state
    assert_eq!(det.filter(0b0001), 0b0001);
    // Trigger first mask
    assert_eq!(det.filter(0b0011), 0b0001);
    // Valid: only second pair, one bit
    assert_eq!(det.filter(0b0100), 0b0100);
    // Trigger second mask
    assert_eq!(det.filter(0b1100), 0b0100);
}

#[test]
fn impossible_detector_is_impossible_method() {
    let det = ImpossibleStateDetector::new(vec![0b0011]);
    assert!(det.is_impossible(0b0011));
    assert!(det.is_impossible(0b0111)); // superset still triggers
    assert!(!det.is_impossible(0b0001));
    assert!(!det.is_impossible(0b0010));
}

#[test]
fn impossible_detector_reset_clears_state() {
    let mut det = ImpossibleStateDetector::new(vec![0b0011]);
    det.filter(0b0001);
    det.reset();
    // After reset, last_valid_state is 0
    assert_eq!(det.filter(0b0011), 0);
}

// ---------------------------------------------------------------------------
// Ghost filter: ButtonDebouncer
// ---------------------------------------------------------------------------

#[test]
fn debouncer_initial_state_zero() {
    let mut deb = ButtonDebouncer::new(Duration::from_millis(10));
    assert_eq!(deb.filter(0), 0);
}

#[test]
fn debouncer_accepts_state_after_threshold() {
    let mut deb = ButtonDebouncer::new(Duration::from_millis(5));
    deb.filter(0b0001);
    std::thread::sleep(Duration::from_millis(10));
    assert_eq!(deb.filter(0b0001), 0b0001);
}

#[test]
fn debouncer_reset_returns_to_zero() {
    let mut deb = ButtonDebouncer::new(Duration::from_millis(5));
    deb.filter(0b0001);
    std::thread::sleep(Duration::from_millis(10));
    deb.filter(0b0001);
    deb.reset();
    assert_eq!(deb.filter(0), 0);
}

// ---------------------------------------------------------------------------
// Ghost filter: GhostInputFilter (composite)
// ---------------------------------------------------------------------------

#[test]
fn ghost_filter_default_creation() {
    let filter = GhostInputFilter::default();
    assert_eq!(filter.ghost_rate(), 0.0);
    assert_eq!(filter.stats().total_samples, 0);
}

#[test]
fn ghost_filter_stats_increment() {
    let mut filter = GhostInputFilter::new();
    filter.filter(0);
    filter.filter(0);
    assert_eq!(filter.stats().total_samples, 2);
    assert_eq!(filter.stats().total_filtered, 0);
}

#[test]
fn ghost_filter_reset_clears_stats() {
    let mut filter = GhostInputFilter::new();
    filter.filter(0);
    filter.filter(0);
    filter.reset();
    assert_eq!(filter.stats().total_samples, 0);
    assert_eq!(filter.ghost_rate(), 0.0);
}

#[test]
fn ghost_filter_with_impossible_mask_filters_state() {
    let config = GhostFilterConfig {
        debounce_threshold: Duration::from_millis(0),
        impossible_masks: vec![0b0011],
    };
    let mut filter = GhostInputFilter::with_config(config);
    // Valid state
    filter.filter(0b0001);
    // Impossible state detected
    let result = filter.filter(0b0011);
    assert_ne!(result, 0b0011);
    assert!(filter.stats().impossible_filtered > 0);
}

#[test]
fn ghost_filter_ghost_rate_calculation() {
    let config = GhostFilterConfig {
        debounce_threshold: Duration::from_millis(0),
        impossible_masks: vec![0b0011],
    };
    let mut filter = GhostInputFilter::with_config(config);
    filter.filter(0b0001); // valid
    filter.filter(0b0011); // ghost
    // 1 out of 2 was filtered
    assert!(filter.ghost_rate() > 0.0);
    assert!(filter.ghost_rate() <= 1.0);
}

// ---------------------------------------------------------------------------
// Ghost filter: preset configs validation
// ---------------------------------------------------------------------------

#[test]
fn all_presets_debounce_in_valid_range() {
    let all_presets = [
        presets::x55_x56_ministick(),
        presets::aggressive(),
        presets::tflight_hotas4(),
        presets::saitek_x55_throttle(),
        presets::saitek_x56_throttle(),
        presets::thrustmaster_warthog(),
        presets::thrustmaster_t16000m(),
        presets::vkb_gladiator(),
    ];
    for config in all_presets {
        let ms = config.debounce_threshold.as_millis();
        assert!((10..=50).contains(&ms), "debounce {ms}ms out of range");
    }
}

#[test]
fn all_presets_masks_have_multiple_bits() {
    let all_presets = [
        presets::x55_x56_ministick(),
        presets::tflight_hotas4(),
        presets::saitek_x55_throttle(),
        presets::saitek_x56_throttle(),
        presets::thrustmaster_warthog(),
        presets::thrustmaster_t16000m(),
        presets::vkb_gladiator(),
    ];
    for config in all_presets {
        for mask in &config.impossible_masks {
            assert!(mask.count_ones() >= 2);
        }
    }
}

#[test]
fn preset_aggressive_has_no_impossible_masks() {
    let config = presets::aggressive();
    assert!(config.impossible_masks.is_empty());
    assert_eq!(config.debounce_threshold, Duration::from_millis(50));
}

// ---------------------------------------------------------------------------
// Vendor ID constants sanity
// ---------------------------------------------------------------------------

#[test]
fn vendor_id_constants_are_distinct() {
    let ids = [
        THRUSTMASTER_VENDOR_ID,
        VKB_VENDOR_ID,
        SAITEK_VENDOR_ID,
        MAD_CATZ_VENDOR_ID,
        LOGITECH_VENDOR_ID,
        HONEYCOMB_VENDOR_ID,
        VIRPIL_VENDOR_ID,
        CH_VENDOR_ID,
        WINWING_VENDOR_ID,
        MOZA_VENDOR_ID,
        MFG_VENDOR_ID,
        REALSIMULATOR_VENDOR_ID,
        BRUNNER_VENDOR_ID,
        MICROSOFT_VENDOR_ID,
    ];
    // All vendor IDs should be unique (no collisions)
    let mut sorted = ids.to_vec();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "duplicate vendor IDs detected");
}

#[test]
fn vendor_ids_nonzero() {
    assert_ne!(THRUSTMASTER_VENDOR_ID, 0);
    assert_ne!(VKB_VENDOR_ID, 0);
    assert_ne!(SAITEK_VENDOR_ID, 0);
    assert_ne!(LOGITECH_VENDOR_ID, 0);
    assert_ne!(VIRPIL_VENDOR_ID, 0);
}

// ---------------------------------------------------------------------------
// Cross-vendor negative tests
// ---------------------------------------------------------------------------

#[test]
fn wrong_vendor_rejects_all_families() {
    let bogus_vid: u16 = 0xDEAD;
    assert!(!is_warthog_device(bogus_vid, WARTHOG_JOYSTICK_PID));
    assert!(!is_t16000m_device(bogus_vid, T16000M_JOYSTICK_PID));
    assert!(!is_tca_airbus_device(
        bogus_vid,
        TCA_SIDESTICK_AIRBUS_PILOT_PID
    ));
    assert!(!is_tca_boeing_device(bogus_vid, TCA_YOKE_BOEING_PID));
    assert!(!is_virpil_device(bogus_vid, VIRPIL_CM2_THROTTLE_PID));
    assert!(!is_ch_device(bogus_vid, CH_PRO_THROTTLE_PID));
    assert!(!is_vpforce_device(bogus_vid, VPFORCE_RHINO_PID_V2));
    assert!(!is_moza_device(bogus_vid, MOZA_AB9_PID));
    assert!(!is_brunner_device(bogus_vid, BRUNNER_CLS_E_YOKE_PID));
    assert!(!is_sidewinder_device(bogus_vid, SIDEWINDER_FFB_PRO_PID));
    assert!(!is_saitek_hotas(bogus_vid, X52_PID));
}

// ---------------------------------------------------------------------------
// HidUsage struct
// ---------------------------------------------------------------------------

#[test]
fn hid_usage_equality_and_clone() {
    let u1 = HidUsage {
        usage_page: 0x01,
        usage: 0x30,
    };
    let u2 = u1;
    assert_eq!(u1, u2);
    let u3 = HidUsage {
        usage_page: 0x01,
        usage: 0x31,
    };
    assert_ne!(u1, u3);
}

// ---------------------------------------------------------------------------
// GhostFilterStats
// ---------------------------------------------------------------------------

#[test]
fn ghost_filter_stats_default() {
    let stats = GhostFilterStats::default();
    assert_eq!(stats.total_samples, 0);
    assert_eq!(stats.total_filtered, 0);
    assert_eq!(stats.debounce_filtered, 0);
    assert_eq!(stats.impossible_filtered, 0);
}

#[test]
fn ghost_filter_stats_equality() {
    let s1 = GhostFilterStats::default();
    let s2 = GhostFilterStats::default();
    assert_eq!(s1, s2);
}

// ---------------------------------------------------------------------------
// GhostFilterConfig
// ---------------------------------------------------------------------------

#[test]
fn ghost_filter_config_default() {
    let config = GhostFilterConfig::default();
    assert_eq!(
        config.debounce_threshold,
        Duration::from_millis(DEFAULT_DEBOUNCE_MS)
    );
    assert!(config.impossible_masks.is_empty());
}

#[test]
fn ghost_filter_config_clone() {
    let config = presets::vkb_gladiator();
    let cloned = config.clone();
    assert_eq!(config.debounce_threshold, cloned.debounce_threshold);
    assert_eq!(config.impossible_masks, cloned.impossible_masks);
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn parse_hex_descriptor(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}
