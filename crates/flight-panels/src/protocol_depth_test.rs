// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Protocol depth tests: property tests, full pipeline integration,
//! error handling, and state machine coverage for panel protocols.

#[cfg(test)]
mod tests {
    use super::super::*;
    use flight_core::rules::{Action, Rule, RuleDefaults, RulesSchema};
    use flight_panels_core::display;
    use flight_panels_core::protocol::{PanelCodec, PanelMessage, PanelProtocol, PanelResponse};
    use flight_panels_saitek::multi_panel::{
        self, LcdDisplay, MultiPanelLedMask, MultiPanelProtocol, encode_segment, led_bits,
    };
    use flight_panels_saitek::radio_panel::{
        RadioDisplay, RadioPanelProtocol, RADIO_PANEL_OUTPUT_BYTES,
    };
    use flight_panels_saitek::switch_panel::{
        GearLedColor, SwitchPanelGearLeds, SwitchPanelProtocol, gear_led_bits,
        SWITCH_PANEL_OUTPUT_BYTES,
    };
    use proptest::prelude::*;
    use std::collections::HashMap;
    use std::time::Duration;

    // ── Property tests: LED state round-trips ────────────────────────────────

    proptest! {
        #[test]
        fn prop_multi_panel_led_mask_roundtrip(v in 0u8..=255u8) {
            let mask = MultiPanelLedMask::from(v);
            prop_assert_eq!(mask.raw(), v);
        }

        #[test]
        fn prop_multi_panel_led_set_clear_idempotent(
            initial in 0u8..=255u8,
            bit_index in 0u8..8u8,
        ) {
            let bit = 1u8 << bit_index;
            let mask = MultiPanelLedMask::from(initial);
            // Setting then clearing should leave other bits unchanged
            let after = mask.set(bit, true).set(bit, false);
            prop_assert_eq!(after.raw() & !bit, initial & !bit);
            prop_assert!(!after.is_set(bit));
        }

        #[test]
        fn prop_gear_led_no_simultaneous_green_red(
            left in 0u8..3u8,
            nose in 0u8..3u8,
            right in 0u8..3u8,
        ) {
            let colors = [GearLedColor::Off, GearLedColor::Green, GearLedColor::Red];
            let leds = SwitchPanelGearLeds::ALL_OFF
                .set_left(colors[left as usize])
                .set_nose(colors[nose as usize])
                .set_right(colors[right as usize]);

            let raw = leds.raw();
            // Green and red must never be set simultaneously for the same gear
            prop_assert!((raw & gear_led_bits::LEFT_GREEN == 0)
                || (raw & gear_led_bits::LEFT_RED == 0));
            prop_assert!((raw & gear_led_bits::NOSE_GREEN == 0)
                || (raw & gear_led_bits::NOSE_RED == 0));
            prop_assert!((raw & gear_led_bits::RIGHT_GREEN == 0)
                || (raw & gear_led_bits::RIGHT_RED == 0));
        }
    }

    // ── Property tests: display digit encoding invariants ────────────────────

    proptest! {
        #[test]
        fn prop_encode_segment_digit_nonzero(d in 0u32..10u32) {
            let c = char::from_digit(d, 10).unwrap();
            prop_assert_ne!(encode_segment(c), 0x00, "digit should not encode to blank");
        }

        #[test]
        fn prop_encode_segment_only_7_bits(c in proptest::char::any()) {
            let encoded = encode_segment(c);
            // 7-segment displays use bits 0-6 only
            prop_assert_eq!(encoded & 0x80, 0x00, "bit 7 should never be set");
        }

        #[test]
        fn prop_lcd_display_from_integer_is_5_bytes(v in i32::MIN..=i32::MAX) {
            let lcd = LcdDisplay::from_integer(v);
            prop_assert_eq!(lcd.as_bytes().len(), 5);
        }

        #[test]
        fn prop_lcd_display_hid_report_is_correct_length(s in "[0-9 \\-]{1,5}") {
            let lcd = LcdDisplay::encode_str(&s);
            let report = lcd.to_hid_report(MultiPanelLedMask::NONE);
            prop_assert_eq!(report.len(), multi_panel::MULTI_PANEL_OUTPUT_BYTES);
            prop_assert_eq!(report[0], 0x00, "report ID must be 0");
        }

        #[test]
        fn prop_radio_display_hid_report_correct_length(
            active_freq in 118_000u32..=136_975u32,
            standby_freq in 118_000u32..=136_975u32,
        ) {
            let disp = RadioDisplay {
                active: LcdDisplay::encode_str(&display::format_com_freq(active_freq)),
                standby: LcdDisplay::encode_str(&display::format_com_freq(standby_freq)),
            };
            let report = disp.to_hid_report();
            prop_assert_eq!(report.len(), RADIO_PANEL_OUTPUT_BYTES);
            prop_assert_eq!(report[0], 0x00);
            // Reserved bytes must be zero
            for i in 11..RADIO_PANEL_OUTPUT_BYTES {
                let msg = format!("reserved byte {i}");
                prop_assert_eq!(report[i], 0x00, "{}", msg);
            }
        }

        #[test]
        fn prop_switch_panel_gear_led_report_length(raw_byte in 0u8..=0x3F) {
            let leds = SwitchPanelGearLeds(raw_byte);
            let report = leds.to_hid_report();
            prop_assert_eq!(report.len(), SWITCH_PANEL_OUTPUT_BYTES);
            prop_assert_eq!(report[0], 0x00);
            prop_assert_eq!(report[1], raw_byte);
        }
    }

    // ── Property tests: display formatter invariants ─────────────────────────

    proptest! {
        #[test]
        fn prop_format_heading_always_5_chars(deg in 0u16..=u16::MAX) {
            let s = display::format_heading(deg);
            prop_assert_eq!(s.len(), 5);
            prop_assert!(s.chars().all(|c| c.is_ascii_digit() || c == ' '));
        }

        #[test]
        fn prop_format_altitude_always_5_chars(feet in i32::MIN..=i32::MAX) {
            let s = display::format_altitude(feet);
            prop_assert_eq!(s.len(), 5);
        }

        #[test]
        fn prop_format_vs_always_5_chars(fpm in i32::MIN..=i32::MAX) {
            let s = display::format_vs(fpm);
            prop_assert_eq!(s.len(), 5);
        }

        #[test]
        fn prop_format_xpdr_always_5_chars(code in 0u16..=u16::MAX) {
            let s = display::format_xpdr(code);
            prop_assert_eq!(s.len(), 5);
        }

        #[test]
        fn prop_format_adf_always_5_chars(freq in 0u16..=u16::MAX) {
            let s = display::format_adf(freq);
            prop_assert_eq!(s.len(), 5);
        }
    }

    // ── Full pipeline: sim variable change → rule eval → LED/display update ──

    #[test]
    fn test_full_pipeline_sim_variable_to_led_update() {
        // Test that rules compile → evaluate → produce LED actions for the pipeline.
        // Use a direct evaluator to avoid PanelManager rate-limiting.
        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('GEAR_GREEN').on()".to_string(),
                    action: "led.panel('GEAR_GREEN').on()".to_string(),
                },
                Rule {
                    when: "!gear_down".to_string(),
                    do_action: "led.panel('GEAR_GREEN').off()".to_string(),
                    action: "led.panel('GEAR_GREEN').off()".to_string(),
                },
                Rule {
                    when: "aoa > 14".to_string(),
                    do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                    action: "led.indexer.blink(rate_hz=6)".to_string(),
                },
            ],
            defaults: None,
        };

        let compiled = rules_schema.compile().unwrap();
        let mut evaluator = RulesEvaluator::new();
        evaluator.initialize_for_program(&compiled.bytecode);
        evaluator.set_min_eval_interval(Duration::ZERO);

        let mut telemetry = HashMap::new();

        // Frame 1: gear down, normal AoA → should produce GEAR_GREEN on
        telemetry.insert("gear_down".to_string(), 1.0);
        telemetry.insert("aoa".to_string(), 5.0);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert!(
            actions.iter().any(|a| matches!(a, Action::LedOn { target } if target == "GEAR_GREEN")),
            "should produce GEAR_GREEN on action, got: {:?}",
            actions
        );

        // Frame 2: gear up → should produce GEAR_GREEN off
        telemetry.insert("gear_down".to_string(), 0.0);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert!(
            actions.iter().any(|a| matches!(a, Action::LedOff { target } if target == "GEAR_GREEN")),
            "should produce GEAR_GREEN off action, got: {:?}",
            actions
        );

        // Frame 3: high AoA → should produce indexer blink
        telemetry.insert("aoa".to_string(), 16.0);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert!(
            actions.iter().any(|a| matches!(a, Action::LedBlink { target, .. } if target == "indexer")),
            "should produce indexer blink action, got: {:?}",
            actions
        );
    }

    #[test]
    fn test_full_pipeline_display_rendering() {
        // Test the display formatting → 7-segment encoding → HID report pipeline
        let freq = 121_500u32; // Guard frequency
        let formatted = display::format_com_freq(freq);
        assert_eq!(formatted, "12150");

        let lcd = LcdDisplay::encode_str(&formatted);
        let report = lcd.to_hid_report(MultiPanelLedMask::NONE);

        // Verify the HID report contains correct 7-segment bytes
        assert_eq!(report[1], encode_segment('1'));
        assert_eq!(report[2], encode_segment('2'));
        assert_eq!(report[3], encode_segment('1'));
        assert_eq!(report[4], encode_segment('5'));
        assert_eq!(report[5], encode_segment('0'));
    }

    // ── Error handling tests ─────────────────────────────────────────────────

    #[test]
    fn test_panel_manager_update_without_rules_is_noop() {
        let mut panel_manager = PanelManager::new();
        let telemetry = HashMap::new();
        // Should not error even without rules loaded
        panel_manager.update(&telemetry).unwrap();
    }

    #[test]
    fn test_panel_manager_load_then_update_empty_telemetry() {
        let mut panel_manager = PanelManager::new();

        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
            defaults: None,
        };
        panel_manager.load_rules(rules_schema).unwrap();

        // Empty telemetry — all variables default to 0.0
        let telemetry = HashMap::new();
        panel_manager.update(&telemetry).unwrap();

        // gear_down defaults to 0.0 → falsy → no GEAR LED on
        let gear_state = panel_manager
            .led_controller()
            .get_led_state(&led::LedTarget::Panel("GEAR".to_string()));
        assert!(
            gear_state.is_none() || gear_state.is_some_and(|s| !s.on),
            "GEAR LED should not be on with empty telemetry"
        );
    }

    #[test]
    fn test_panel_manager_fault_and_clear_cycle() {
        // Test fault indication via direct LED controller to avoid rate-limiting
        let mut led_ctrl = LedController::new();
        led_ctrl.set_min_interval(Duration::ZERO);

        // Trigger fault indication
        let fault_actions = vec![
            Action::LedOn {
                target: "MASTER_WARNING".to_string(),
            },
        ];
        led_ctrl.execute_actions(&fault_actions).unwrap();

        let fault_state = led_ctrl.get_led_state(&led::LedTarget::Panel("MASTER_WARNING".to_string()));
        assert!(
            fault_state.is_some_and(|s| s.on),
            "MASTER_WARNING should be on after fault"
        );

        // Clear fault
        let clear_actions = vec![
            Action::LedOff {
                target: "MASTER_WARNING".to_string(),
            },
        ];
        led_ctrl.execute_actions(&clear_actions).unwrap();

        let fault_state = led_ctrl.get_led_state(&led::LedTarget::Panel("MASTER_WARNING".to_string()));
        assert!(
            fault_state.is_some_and(|s| !s.on),
            "MASTER_WARNING should be off after clear"
        );
    }

    #[test]
    fn test_panel_manager_soft_stop_indication() {
        let mut panel_manager = PanelManager::new();
        panel_manager.trigger_soft_stop_indication().unwrap();

        let state = panel_manager
            .led_controller()
            .get_led_state(&led::LedTarget::Panel("SOFT_STOP_INDICATOR".to_string()));
        assert!(state.is_some_and(|s| s.on && s.brightness == 1.0));
    }

    #[test]
    fn test_panel_manager_reload_rules() {
        let mut panel_manager = PanelManager::new();

        // Load first rule set
        let schema1 = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
            defaults: None,
        };
        panel_manager.load_rules(schema1).unwrap();

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);
        panel_manager.update(&telemetry).unwrap();

        // Reload with different rules
        let schema2 = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "flaps_deployed".to_string(),
                do_action: "led.panel('FLAPS').on()".to_string(),
                action: "led.panel('FLAPS').on()".to_string(),
            }],
            defaults: None,
        };
        panel_manager.load_rules(schema2).unwrap();

        telemetry.insert("flaps_deployed".to_string(), 1.0);
        panel_manager.update(&telemetry).unwrap();
    }

    // ── Malformed HID report handling ────────────────────────────────────────

    #[test]
    fn test_multi_panel_protocol_handles_empty_input() {
        let proto = MultiPanelProtocol;
        assert!(proto.parse_input(&[]).is_none());
    }

    #[test]
    fn test_radio_panel_protocol_handles_single_byte() {
        let proto = RadioPanelProtocol;
        assert!(proto.parse_input(&[0x00]).is_none());
        assert!(proto.parse_input(&[0xFF]).is_none());
    }

    #[test]
    fn test_switch_panel_protocol_handles_short_input() {
        let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
        assert!(proto.parse_input(&[]).is_none());
        assert!(proto.parse_input(&[0x00]).is_none());
        assert!(proto.parse_input(&[0x00, 0xFF]).is_none());
    }

    #[test]
    fn test_multi_panel_protocol_all_bits_set() {
        let proto = MultiPanelProtocol;
        // All bits set — should parse without panicking
        let data = [0x00u8, 0xFF, 0xFF];
        let events = proto.parse_input(&data).unwrap();
        assert!(!events.is_empty(), "all-bits-set report should produce events");
    }

    #[test]
    fn test_radio_panel_protocol_all_bits_set() {
        let proto = RadioPanelProtocol;
        let data = [0x00u8, 0xFF, 0xFF];
        let events = proto.parse_input(&data).unwrap();
        // Reserved mode 7 may produce None for mode, but should still parse buttons
        assert!(!events.is_empty());
    }

    #[test]
    fn test_switch_panel_protocol_all_bits_set() {
        let proto = SwitchPanelProtocol::new(Duration::from_millis(5));
        let data = [0x00u8, 0xFF, 0xFF];
        let events = proto.parse_input(&data).unwrap();
        assert!(!events.is_empty());
    }
}
