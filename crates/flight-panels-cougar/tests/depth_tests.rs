// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the Cougar MFD panel protocol.
//!
//! These integration-level tests exercise the public API of `flight-panels-cougar`
//! covering MFD button indexing, button-state bitfield parsing, OSB labelling,
//! page/display management, protocol metadata, HID report building, and the
//! verify-test result analysis helpers.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use flight_panels_cougar::cougar::{
    CougarMfdType, CougarMfdWriter, CougarVerifyStep, CougarVerifyStepResult,
    CougarVerifyTestResult, MfdLedState,
};
use flight_panels_cougar::mfd::{
    COUGAR_VID, CougarMfdProtocol, MfdButton, MfdButtonState, MfdDisplay, MfdPage, OSB_COUNT,
    OSB_NAMES, OSBS_PER_SIDE, OsbLabel,
};

use flight_panels_core::protocol::{PanelEvent, PanelProtocol};

// ═══════════════════════════════════════════════════════════════════════════════
// MfdButton
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn button_all_array_length_equals_osb_count() {
    assert_eq!(MfdButton::ALL.len(), OSB_COUNT);
}

#[test]
fn button_indices_are_contiguous_zero_to_nineteen() {
    let indices: Vec<usize> = MfdButton::ALL.iter().map(|b| b.index()).collect();
    let expected: Vec<usize> = (0..OSB_COUNT).collect();
    assert_eq!(indices, expected);
}

#[test]
fn button_from_index_returns_none_for_out_of_range() {
    assert!(MfdButton::from_index(OSB_COUNT).is_none());
    assert!(MfdButton::from_index(usize::MAX).is_none());
}

#[test]
fn button_from_index_roundtrips_all_variants() {
    for i in 0..OSB_COUNT {
        let btn = MfdButton::from_index(i).unwrap();
        assert_eq!(btn.index(), i);
    }
}

#[test]
fn button_indices_are_unique() {
    let mut seen = HashSet::new();
    for btn in MfdButton::ALL {
        assert!(seen.insert(btn.index()), "duplicate index {}", btn.index());
    }
}

#[test]
fn button_osb_names_sequential_osb1_through_osb20() {
    for (i, btn) in MfdButton::ALL.iter().enumerate() {
        assert_eq!(btn.osb_name(), format!("OSB{}", i + 1));
    }
}

#[test]
fn button_osb_names_table_matches_method() {
    for (i, btn) in MfdButton::ALL.iter().enumerate() {
        assert_eq!(OSB_NAMES[i], btn.osb_name());
    }
}

#[test]
fn button_side_grouping_top() {
    let top = [
        MfdButton::Top1,
        MfdButton::Top2,
        MfdButton::Top3,
        MfdButton::Top4,
        MfdButton::Top5,
    ];
    for (offset, btn) in top.iter().enumerate() {
        assert_eq!(btn.index(), offset);
    }
}

#[test]
fn button_side_grouping_right() {
    let right = [
        MfdButton::Right1,
        MfdButton::Right2,
        MfdButton::Right3,
        MfdButton::Right4,
        MfdButton::Right5,
    ];
    for (offset, btn) in right.iter().enumerate() {
        assert_eq!(btn.index(), OSBS_PER_SIDE + offset);
    }
}

#[test]
fn button_side_grouping_bottom() {
    let bottom = [
        MfdButton::Bottom1,
        MfdButton::Bottom2,
        MfdButton::Bottom3,
        MfdButton::Bottom4,
        MfdButton::Bottom5,
    ];
    for (offset, btn) in bottom.iter().enumerate() {
        assert_eq!(btn.index(), 2 * OSBS_PER_SIDE + offset);
    }
}

#[test]
fn button_side_grouping_left() {
    let left = [
        MfdButton::Left1,
        MfdButton::Left2,
        MfdButton::Left3,
        MfdButton::Left4,
        MfdButton::Left5,
    ];
    for (offset, btn) in left.iter().enumerate() {
        assert_eq!(btn.index(), 3 * OSBS_PER_SIDE + offset);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MfdButtonState — bitfield
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn state_default_no_buttons_pressed() {
    let s = MfdButtonState::default();
    assert_eq!(s.0, 0);
    for btn in MfdButton::ALL {
        assert!(!s.is_pressed(btn));
    }
}

#[test]
fn state_set_single_button() {
    let mut s = MfdButtonState::default();
    s.set(MfdButton::Top3, true);
    assert!(s.is_pressed(MfdButton::Top3));
    assert!(!s.is_pressed(MfdButton::Top1));
}

#[test]
fn state_clear_button() {
    let mut s = MfdButtonState::default();
    s.set(MfdButton::Right2, true);
    assert!(s.is_pressed(MfdButton::Right2));
    s.set(MfdButton::Right2, false);
    assert!(!s.is_pressed(MfdButton::Right2));
}

#[test]
fn state_set_all_buttons() {
    let mut s = MfdButtonState::default();
    for btn in MfdButton::ALL {
        s.set(btn, true);
    }
    for btn in MfdButton::ALL {
        assert!(s.is_pressed(btn));
    }
    // Only 20 low bits should be set.
    assert_eq!(s.0, 0x000F_FFFF);
}

#[test]
fn state_double_set_idempotent() {
    let mut s = MfdButtonState::default();
    s.set(MfdButton::Bottom4, true);
    let val_after_first = s.0;
    s.set(MfdButton::Bottom4, true);
    assert_eq!(s.0, val_after_first);
}

#[test]
fn state_double_clear_idempotent() {
    let mut s = MfdButtonState::default();
    s.set(MfdButton::Left1, false);
    assert_eq!(s.0, 0);
}

#[test]
fn state_from_bytes_all_zeros() {
    let s = MfdButtonState::from_bytes(&[0, 0, 0]);
    assert_eq!(s.0, 0);
}

#[test]
fn state_from_bytes_all_ones_masked_to_20_bits() {
    let s = MfdButtonState::from_bytes(&[0xFF, 0xFF, 0xFF]);
    assert_eq!(s.0, 0x000F_FFFF);
}

#[test]
fn state_from_bytes_too_short_returns_zero() {
    assert_eq!(MfdButtonState::from_bytes(&[]).0, 0);
    assert_eq!(MfdButtonState::from_bytes(&[0xFF]).0, 0);
    assert_eq!(MfdButtonState::from_bytes(&[0xFF, 0xFF]).0, 0);
}

#[test]
fn state_from_bytes_ignores_extra_bytes() {
    let s = MfdButtonState::from_bytes(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    assert!(s.is_pressed(MfdButton::Top1));
    assert_eq!(s.0, 1);
}

#[test]
fn state_from_bytes_specific_bit_positions() {
    // bit 0 = Top1, bit 9 = Right5, bit 19 = Left5
    let data = [0x01, 0x02, 0x08]; // 0x080201
    let s = MfdButtonState::from_bytes(&data);
    assert!(s.is_pressed(MfdButton::Top1)); // bit 0
    assert!(s.is_pressed(MfdButton::Right5)); // bit 9
    assert!(s.is_pressed(MfdButton::Left5)); // bit 19
    assert!(!s.is_pressed(MfdButton::Top2)); // bit 1
}

#[test]
fn state_diff_no_change() {
    let a = MfdButtonState(0b1010);
    let b = MfdButtonState(0b1010);
    assert!(a.diff(&b).is_empty());
}

#[test]
fn state_diff_single_press() {
    let old = MfdButtonState(0);
    let new = MfdButtonState(1 << MfdButton::Bottom1.index());
    let d = old.diff(&new);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0], (MfdButton::Bottom1, true));
}

#[test]
fn state_diff_single_release() {
    let old = MfdButtonState(1 << MfdButton::Top5.index());
    let new = MfdButtonState(0);
    let d = old.diff(&new);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0], (MfdButton::Top5, false));
}

#[test]
fn state_diff_multiple_changes() {
    let old = MfdButtonState(0b11); // Top1 + Top2
    let new = MfdButtonState(0b10); // Top2 only
    let d = old.diff(&new);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0], (MfdButton::Top1, false));
}

#[test]
fn state_diff_simultaneous_press_and_release() {
    let old = MfdButtonState(1 << 0); // Top1
    let new = MfdButtonState(1 << 1); // Top2
    let d = old.diff(&new);
    assert_eq!(d.len(), 2);
    let released: Vec<_> = d.iter().filter(|(_, pressed)| !pressed).collect();
    let pressed: Vec<_> = d.iter().filter(|(_, pressed)| *pressed).collect();
    assert_eq!(released.len(), 1);
    assert_eq!(pressed.len(), 1);
    assert_eq!(released[0].0, MfdButton::Top1);
    assert_eq!(pressed[0].0, MfdButton::Top2);
}

// ═══════════════════════════════════════════════════════════════════════════════
// OsbLabel / MfdPage
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn osb_label_default_is_empty_inactive() {
    let label = OsbLabel::default();
    assert_eq!(label.text, "");
    assert!(!label.active);
}

#[test]
fn page_new_blank_has_correct_name() {
    let page = MfdPage::new("TAD");
    assert_eq!(page.name, "TAD");
}

#[test]
fn page_new_blank_labels_all_empty() {
    let page = MfdPage::new("BLANK");
    for btn in MfdButton::ALL {
        let lbl = page.label(btn);
        assert_eq!(lbl.text, "");
        assert!(!lbl.active);
    }
}

#[test]
fn page_set_label_updates_correct_button() {
    let mut page = MfdPage::new("MAIN");
    page.set_label(MfdButton::Right3, "TGP", true);
    let lbl = page.label(MfdButton::Right3);
    assert_eq!(lbl.text, "TGP");
    assert!(lbl.active);
    // Neighbouring button unaffected.
    assert_eq!(page.label(MfdButton::Right2).text, "");
}

#[test]
fn page_overwrite_label() {
    let mut page = MfdPage::new("P");
    page.set_label(MfdButton::Top1, "A", false);
    page.set_label(MfdButton::Top1, "B", true);
    assert_eq!(page.label(MfdButton::Top1).text, "B");
    assert!(page.label(MfdButton::Top1).active);
}

// ═══════════════════════════════════════════════════════════════════════════════
// MfdDisplay — page navigation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn display_new_starts_with_one_default_page() {
    let d = MfdDisplay::new();
    assert_eq!(d.page_count(), 1);
    assert_eq!(d.current_index(), 0);
    assert_eq!(d.current_page().name, "DEFAULT");
}

#[test]
fn display_default_trait_same_as_new() {
    let d = MfdDisplay::default();
    assert_eq!(d.page_count(), 1);
    assert_eq!(d.current_page().name, "DEFAULT");
}

#[test]
fn display_add_page_returns_correct_index() {
    let mut d = MfdDisplay::new();
    assert_eq!(d.add_page(MfdPage::new("A")), 1);
    assert_eq!(d.add_page(MfdPage::new("B")), 2);
}

#[test]
fn display_select_page_valid() {
    let mut d = MfdDisplay::new();
    d.add_page(MfdPage::new("X"));
    d.select_page(1);
    assert_eq!(d.current_index(), 1);
    assert_eq!(d.current_page().name, "X");
}

#[test]
fn display_select_page_out_of_range_is_noop() {
    let mut d = MfdDisplay::new();
    d.select_page(100);
    assert_eq!(d.current_index(), 0);
}

#[test]
fn display_next_page_wraps_around() {
    let mut d = MfdDisplay::new();
    d.add_page(MfdPage::new("P1"));
    d.next_page();
    assert_eq!(d.current_index(), 1);
    d.next_page();
    assert_eq!(d.current_index(), 0); // wrap
}

#[test]
fn display_prev_page_wraps_around() {
    let mut d = MfdDisplay::new();
    d.add_page(MfdPage::new("P1"));
    d.prev_page();
    assert_eq!(d.current_index(), 1); // wrap backward
    d.prev_page();
    assert_eq!(d.current_index(), 0);
}

#[test]
fn display_next_prev_cycle_three_pages() {
    let mut d = MfdDisplay::new();
    d.add_page(MfdPage::new("A"));
    d.add_page(MfdPage::new("B"));
    // Forward cycle: DEFAULT -> A -> B -> DEFAULT
    for expected in [1, 2, 0] {
        d.next_page();
        assert_eq!(d.current_index(), expected);
    }
    // Backward cycle: DEFAULT -> B -> A -> DEFAULT
    for expected in [2, 1, 0] {
        d.prev_page();
        assert_eq!(d.current_index(), expected);
    }
}

#[test]
fn display_current_page_mut_allows_modification() {
    let mut d = MfdDisplay::new();
    d.current_page_mut().set_label(MfdButton::Top1, "MOD", true);
    let lbl = d.current_page().label(MfdButton::Top1);
    assert_eq!(lbl.text, "MOD");
}

// ═══════════════════════════════════════════════════════════════════════════════
// CougarMfdType
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mfd_type_from_product_id_all_variants() {
    assert_eq!(
        CougarMfdType::from_product_id(0x0404),
        Some(CougarMfdType::MfdLeft)
    );
    assert_eq!(
        CougarMfdType::from_product_id(0x0405),
        Some(CougarMfdType::MfdRight)
    );
    assert_eq!(
        CougarMfdType::from_product_id(0x0406),
        Some(CougarMfdType::MfdCenter)
    );
}

#[test]
fn mfd_type_from_product_id_unknown_returns_none() {
    assert!(CougarMfdType::from_product_id(0x0000).is_none());
    assert!(CougarMfdType::from_product_id(0xFFFF).is_none());
}

#[test]
fn mfd_type_names_are_distinct() {
    let names: HashSet<&str> = [
        CougarMfdType::MfdLeft.name(),
        CougarMfdType::MfdRight.name(),
        CougarMfdType::MfdCenter.name(),
    ]
    .into_iter()
    .collect();
    assert_eq!(names.len(), 3);
}

#[test]
fn mfd_type_left_right_have_25_leds() {
    assert_eq!(CougarMfdType::MfdLeft.led_mapping().len(), 25);
    assert_eq!(CougarMfdType::MfdRight.led_mapping().len(), 25);
}

#[test]
fn mfd_type_center_has_13_leds() {
    assert_eq!(CougarMfdType::MfdCenter.led_mapping().len(), 13);
}

#[test]
fn mfd_type_led_mappings_start_with_osb() {
    for mfd_type in [
        CougarMfdType::MfdLeft,
        CougarMfdType::MfdRight,
        CougarMfdType::MfdCenter,
    ] {
        let mapping = mfd_type.led_mapping();
        assert!(
            mapping[0].starts_with("OSB"),
            "{:?} mapping doesn't start with OSB",
            mfd_type
        );
    }
}

#[test]
fn mfd_type_center_has_power_led() {
    let mapping = CougarMfdType::MfdCenter.led_mapping();
    assert!(
        mapping.contains(&"POWER"),
        "Center MFD should have POWER LED"
    );
}

#[test]
fn mfd_type_left_right_have_brightness_contrast_leds() {
    for mfd in [CougarMfdType::MfdLeft, CougarMfdType::MfdRight] {
        let mapping = mfd.led_mapping();
        assert!(mapping.contains(&"BRIGHTNESS"));
        assert!(mapping.contains(&"CONTRAST"));
    }
}

#[test]
fn mfd_type_verify_pattern_not_empty() {
    for mfd in [
        CougarMfdType::MfdLeft,
        CougarMfdType::MfdRight,
        CougarMfdType::MfdCenter,
    ] {
        assert!(!mfd.verify_pattern().is_empty());
    }
}

#[test]
fn mfd_type_verify_pattern_ends_with_all_off() {
    for mfd in [
        CougarMfdType::MfdLeft,
        CougarMfdType::MfdRight,
        CougarMfdType::MfdCenter,
    ] {
        let pattern = mfd.verify_pattern();
        assert!(
            matches!(pattern.last(), Some(CougarVerifyStep::AllOff)),
            "{:?} verify pattern should end with AllOff",
            mfd
        );
    }
}

#[test]
fn mfd_type_verify_pattern_contains_led_on_and_delay() {
    for mfd in [CougarMfdType::MfdLeft, CougarMfdType::MfdCenter] {
        let pattern = mfd.verify_pattern();
        assert!(
            pattern
                .iter()
                .any(|s| matches!(s, CougarVerifyStep::LedOn(_)))
        );
        assert!(
            pattern
                .iter()
                .any(|s| matches!(s, CougarVerifyStep::Delay(_)))
        );
    }
}

#[test]
fn mfd_type_left_right_patterns_are_identical_length() {
    let left = CougarMfdType::MfdLeft.verify_pattern();
    let right = CougarMfdType::MfdRight.verify_pattern();
    assert_eq!(left.len(), right.len());
}

// ═══════════════════════════════════════════════════════════════════════════════
// CougarMfdProtocol — PanelProtocol implementation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn protocol_name_matches_mfd_type() {
    for mfd in [
        CougarMfdType::MfdLeft,
        CougarMfdType::MfdRight,
        CougarMfdType::MfdCenter,
    ] {
        let proto = CougarMfdProtocol::new(mfd);
        assert_eq!(proto.name(), mfd.name());
    }
}

#[test]
fn protocol_vendor_id_is_thrustmaster() {
    let proto = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
    assert_eq!(proto.vendor_id(), COUGAR_VID);
    assert_eq!(proto.vendor_id(), 0x044F);
}

#[test]
fn protocol_product_ids_match_enum_discriminants() {
    assert_eq!(
        CougarMfdProtocol::new(CougarMfdType::MfdLeft).product_id(),
        0x0404
    );
    assert_eq!(
        CougarMfdProtocol::new(CougarMfdType::MfdRight).product_id(),
        0x0405
    );
    assert_eq!(
        CougarMfdProtocol::new(CougarMfdType::MfdCenter).product_id(),
        0x0406
    );
}

#[test]
fn protocol_led_names_count_matches_mfd_type() {
    for mfd in [
        CougarMfdType::MfdLeft,
        CougarMfdType::MfdRight,
        CougarMfdType::MfdCenter,
    ] {
        let proto = CougarMfdProtocol::new(mfd);
        assert_eq!(proto.led_names().len(), mfd.led_mapping().len());
    }
}

#[test]
fn protocol_output_report_size_is_four() {
    let proto = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
    assert_eq!(proto.output_report_size(), 4);
}

#[test]
fn protocol_parse_input_too_short_returns_none() {
    let proto = CougarMfdProtocol::new(CougarMfdType::MfdRight);
    assert!(proto.parse_input(&[]).is_none());
    assert!(proto.parse_input(&[0x00]).is_none());
    assert!(proto.parse_input(&[0x00, 0x00]).is_none());
}

#[test]
fn protocol_parse_input_no_buttons_returns_empty_vec() {
    let proto = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
    let events = proto.parse_input(&[0x00, 0x00, 0x00]).unwrap();
    assert!(events.is_empty());
}

#[test]
fn protocol_parse_input_single_button() {
    let proto = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
    // bit 0 = Top1 → OSB1
    let events = proto.parse_input(&[0x01, 0x00, 0x00]).unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        PanelEvent::ButtonPress { name: "OSB1" }
    ));
}

#[test]
fn protocol_parse_input_multiple_buttons() {
    let proto = CougarMfdProtocol::new(CougarMfdType::MfdCenter);
    // bits 0, 4, 9 = Top1, Top5, Right5
    let data = [0x11, 0x02, 0x00]; // 0x000211
    let events = proto.parse_input(&data).unwrap();
    assert_eq!(events.len(), 3);
    let names: Vec<&str> = events
        .iter()
        .map(|e| match e {
            PanelEvent::ButtonPress { name } => *name,
            _ => panic!("unexpected event type"),
        })
        .collect();
    assert!(names.contains(&"OSB1"));
    assert!(names.contains(&"OSB5"));
    assert!(names.contains(&"OSB10"));
}

#[test]
fn protocol_parse_input_all_buttons_pressed() {
    let proto = CougarMfdProtocol::new(CougarMfdType::MfdRight);
    let events = proto.parse_input(&[0xFF, 0xFF, 0x0F]).unwrap();
    assert_eq!(events.len(), OSB_COUNT);
}

// ═══════════════════════════════════════════════════════════════════════════════
// CougarVerifyTestResult — latency helpers
// ═══════════════════════════════════════════════════════════════════════════════

fn make_step_result(index: usize, actual_ms: u64, success: bool) -> CougarVerifyStepResult {
    CougarVerifyStepResult {
        step_index: index,
        expected_latency: Duration::from_millis(20),
        actual_latency: Duration::from_millis(actual_ms),
        success,
        error: None,
    }
}

fn make_test_result(steps: Vec<CougarVerifyStepResult>, success: bool) -> CougarVerifyTestResult {
    CougarVerifyTestResult {
        mfd_path: "/dev/hidraw0".to_string(),
        total_duration: Duration::from_millis(500),
        step_results: steps,
        success,
    }
}

#[test]
fn verify_result_meets_latency_all_under_20ms() {
    let r = make_test_result(
        vec![
            make_step_result(0, 5, true),
            make_step_result(1, 10, true),
            make_step_result(2, 20, true),
        ],
        true,
    );
    assert!(r.meets_latency_requirement());
}

#[test]
fn verify_result_fails_latency_when_one_exceeds_20ms() {
    let r = make_test_result(
        vec![make_step_result(0, 5, true), make_step_result(1, 21, false)],
        false,
    );
    assert!(!r.meets_latency_requirement());
}

#[test]
fn verify_result_max_latency() {
    let r = make_test_result(
        vec![
            make_step_result(0, 3, true),
            make_step_result(1, 18, true),
            make_step_result(2, 7, true),
        ],
        true,
    );
    assert_eq!(r.max_latency(), Duration::from_millis(18));
}

#[test]
fn verify_result_max_latency_empty_steps() {
    let r = make_test_result(vec![], true);
    assert_eq!(r.max_latency(), Duration::ZERO);
}

#[test]
fn verify_result_avg_latency() {
    let r = make_test_result(
        vec![
            make_step_result(0, 6, true),
            make_step_result(1, 12, true),
            make_step_result(2, 12, true),
        ],
        true,
    );
    assert_eq!(r.avg_latency(), Duration::from_millis(10));
}

#[test]
fn verify_result_avg_latency_empty_steps() {
    let r = make_test_result(vec![], true);
    assert_eq!(r.avg_latency(), Duration::ZERO);
}

// ═══════════════════════════════════════════════════════════════════════════════
// CougarMfdWriter — HID report builders (no hardware needed)
// ═══════════════════════════════════════════════════════════════════════════════

fn make_led_state(index: u8, brightness: f32, is_on: bool) -> MfdLedState {
    MfdLedState {
        led_index: index,
        brightness,
        is_on,
        blink_rate: None,
        last_blink_toggle: Instant::now(),
        last_write: Instant::now(),
    }
}

fn make_writer() -> CougarMfdWriter {
    use flight_hid::HidAdapter;
    use flight_watchdog::WatchdogSystem;
    use std::sync::{Arc, Mutex};

    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    let hid = HidAdapter::new(watchdog);
    CougarMfdWriter::new(hid)
}

#[test]
fn writer_left_report_size_is_32() {
    let w = make_writer();
    let report = w
        .build_mfd_left_report(&make_led_state(0, 1.0, true))
        .unwrap();
    assert_eq!(report.len(), 32);
}

#[test]
fn writer_right_report_size_is_32() {
    let w = make_writer();
    let report = w
        .build_mfd_right_report(&make_led_state(0, 1.0, true))
        .unwrap();
    assert_eq!(report.len(), 32);
}

#[test]
fn writer_center_report_size_is_16() {
    let w = make_writer();
    let report = w
        .build_mfd_center_report(&make_led_state(0, 1.0, true))
        .unwrap();
    assert_eq!(report.len(), 16);
}

#[test]
fn writer_report_id_is_0x01() {
    let w = make_writer();
    assert_eq!(
        w.build_mfd_left_report(&make_led_state(0, 1.0, true))
            .unwrap()[0],
        0x01
    );
    assert_eq!(
        w.build_mfd_right_report(&make_led_state(0, 1.0, true))
            .unwrap()[0],
        0x01
    );
    assert_eq!(
        w.build_mfd_center_report(&make_led_state(0, 1.0, true))
            .unwrap()[0],
        0x01
    );
}

#[test]
fn writer_led_off_writes_zero_brightness() {
    let w = make_writer();
    let report = w
        .build_mfd_left_report(&make_led_state(0, 0.8, false))
        .unwrap();
    assert_eq!(report[1], 0);
}

#[test]
fn writer_led_full_brightness_writes_255() {
    let w = make_writer();
    let report = w
        .build_mfd_left_report(&make_led_state(0, 1.0, true))
        .unwrap();
    assert_eq!(report[1], 255);
}

#[test]
fn writer_led_half_brightness_writes_127() {
    let w = make_writer();
    let report = w
        .build_mfd_left_report(&make_led_state(0, 0.5, true))
        .unwrap();
    assert_eq!(report[1], 127);
}

#[test]
fn writer_led_index_written_at_correct_offset() {
    let w = make_writer();
    let led = make_led_state(5, 1.0, true);
    let report = w.build_mfd_left_report(&led).unwrap();
    // report[0] = report ID, report[1+5] = LED index 5
    assert_eq!(report[6], 255);
    // Other LED positions remain zero.
    assert_eq!(report[1], 0);
    assert_eq!(report[7], 0);
}

#[test]
fn writer_center_led_out_of_range_ignored() {
    let w = make_writer();
    // Center MFD only has 13 LEDs; index 14 should be silently ignored.
    let led = make_led_state(14, 1.0, true);
    let report = w.build_mfd_center_report(&led).unwrap();
    // All bytes beyond report ID should be zero.
    assert!(report[1..].iter().all(|&b| b == 0));
}

#[test]
fn writer_left_led_out_of_range_ignored() {
    let w = make_writer();
    // Left MFD has 25 LEDs; index 26 should be silently ignored.
    let led = make_led_state(26, 1.0, true);
    let report = w.build_mfd_left_report(&led).unwrap();
    assert!(report[1..].iter().all(|&b| b == 0));
}

#[test]
fn writer_default_rate_limit_is_8ms() {
    let w = make_writer();
    assert_eq!(w.get_min_write_interval(), Duration::from_millis(8));
}

#[test]
fn writer_no_latency_stats_initially() {
    let w = make_writer();
    assert!(w.get_latency_stats().is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// Constants and cross-cutting concerns
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn cougar_vid_is_thrustmaster() {
    assert_eq!(COUGAR_VID, 0x044F);
}

#[test]
fn osb_count_is_twenty() {
    assert_eq!(OSB_COUNT, 20);
}

#[test]
fn osbs_per_side_times_four_equals_osb_count() {
    assert_eq!(OSBS_PER_SIDE * 4, OSB_COUNT);
}

#[test]
fn osb_names_length_equals_osb_count() {
    assert_eq!(OSB_NAMES.len(), OSB_COUNT);
}
