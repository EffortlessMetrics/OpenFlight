// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the panel protocol core (generic panel driver framework).
//!
//! Coverage areas:
//! 1. Panel registration — register, unregister, discovery, capabilities, types, hot-plug
//! 2. Display protocol — 7-segment, LCD, LED on/off/blink, multi-color, refresh, buffering
//! 3. Input protocol — switch, encoder, button, hat, analog, debounce
//! 4. Variable binding — sim→display, sim→LED, button→command, encoder→variable, bidir
//! 5. State machine — init→ready→active→error→recovery, transitions, recovery, firmware
//! 6. Integration — roundtrip, profile config, multi-panel coordination

#[cfg(test)]
mod tests {
    use crate::display::*;
    use crate::evaluator::RulesEvaluator;
    use crate::led::{LedBackend, LedController, LedState, LedTarget};
    use crate::protocol::*;
    use flight_core::rules::{Action, Rule, RulesSchema};
    use flight_core::Result;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // ═══════════════════════════════════════════════════════════════════════════
    // Helper infrastructure
    // ═══════════════════════════════════════════════════════════════════════════

    /// A mock panel that supports configurable capabilities.
    struct ConfigurablePanel {
        name: &'static str,
        vendor_id: u16,
        product_id: u16,
        leds: &'static [&'static str],
        report_size: usize,
        /// Bitmap bit positions for buttons, indexed by report byte offset.
        button_map: Vec<(&'static str, usize, u8)>, // (name, byte_offset, bit_mask)
    }

    impl PanelProtocol for ConfigurablePanel {
        fn name(&self) -> &str {
            self.name
        }
        fn vendor_id(&self) -> u16 {
            self.vendor_id
        }
        fn product_id(&self) -> u16 {
            self.product_id
        }
        fn led_names(&self) -> &[&'static str] {
            self.leds
        }
        fn output_report_size(&self) -> usize {
            self.report_size
        }
        fn parse_input(&self, data: &[u8]) -> Option<Vec<PanelEvent>> {
            if data.len() < 2 {
                return None;
            }
            let mut events = Vec::new();
            for &(name, byte_off, mask) in &self.button_map {
                if byte_off < data.len() && data[byte_off] & mask != 0 {
                    events.push(PanelEvent::ButtonPress { name });
                }
            }
            Some(events)
        }
    }

    fn radio_panel() -> ConfigurablePanel {
        ConfigurablePanel {
            name: "Saitek Radio Panel",
            vendor_id: 0x06A3,
            product_id: 0x0D05,
            leds: &["COM1_ACTIVE", "COM1_STBY", "COM2_ACTIVE", "COM2_STBY"],
            report_size: 23,
            button_map: vec![("ACT_STBY", 1, 0x01), ("DME", 1, 0x02)],
        }
    }

    fn multi_panel() -> ConfigurablePanel {
        ConfigurablePanel {
            name: "Saitek Multi Panel",
            vendor_id: 0x06A3,
            product_id: 0x0D06,
            leds: &["AP", "HDG", "NAV", "APR", "REV", "ALT", "VS", "IAS"],
            report_size: 13,
            button_map: vec![
                ("AP_BTN", 1, 0x01),
                ("HDG_BTN", 1, 0x02),
                ("ALT_BTN", 1, 0x04),
            ],
        }
    }

    fn switch_panel() -> ConfigurablePanel {
        ConfigurablePanel {
            name: "Saitek Switch Panel",
            vendor_id: 0x06A3,
            product_id: 0x0D67,
            leds: &["GEAR_N", "GEAR_L", "GEAR_R"],
            report_size: 3,
            button_map: vec![("MASTER_BAT", 0, 0x01), ("MASTER_ALT", 0, 0x02)],
        }
    }

    /// Simple panel registry for testing registration / unregistration.
    struct PanelRegistry {
        panels: HashMap<String, Box<dyn PanelProtocol>>,
    }

    impl PanelRegistry {
        fn new() -> Self {
            Self {
                panels: HashMap::new(),
            }
        }
        fn register(&mut self, path: String, panel: Box<dyn PanelProtocol>) -> bool {
            if self.panels.contains_key(&path) {
                return false;
            }
            self.panels.insert(path, panel);
            true
        }
        fn unregister(&mut self, path: &str) -> bool {
            self.panels.remove(path).is_some()
        }
        fn discover(&self) -> Vec<&str> {
            self.panels.keys().map(|s| s.as_str()).collect()
        }
        fn capabilities(&self, path: &str) -> Option<(&str, &[&'static str], usize)> {
            self.panels
                .get(path)
                .map(|p| (p.name(), p.led_names(), p.output_report_size()))
        }
        fn get(&self, path: &str) -> Option<&dyn PanelProtocol> {
            self.panels.get(path).map(|p| p.as_ref())
        }
        fn count(&self) -> usize {
            self.panels.len()
        }
    }

    type WriteLog = Arc<Mutex<Vec<(LedTarget, bool, f32)>>>;

    /// Recording LED backend that captures all writes.
    struct RecordingBackend {
        writes: WriteLog,
    }

    impl RecordingBackend {
        fn new() -> (Self, WriteLog) {
            let writes: WriteLog = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    writes: Arc::clone(&writes),
                },
                writes,
            )
        }
    }

    impl LedBackend for RecordingBackend {
        fn write(&mut self, target: &LedTarget, state: &LedState) -> Result<()> {
            self.writes
                .lock()
                .unwrap()
                .push((target.clone(), state.on, state.brightness));
            Ok(())
        }
    }

    /// Panel state machine for lifecycle testing.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum PanelState {
        Init,
        Ready,
        Active,
        Error,
        Recovery,
    }

    struct PanelStateMachine {
        state: PanelState,
        firmware_version: Option<(u8, u8, u8)>,
        error_count: u32,
        recovery_attempts: u32,
        max_recovery_attempts: u32,
    }

    impl PanelStateMachine {
        fn new() -> Self {
            Self {
                state: PanelState::Init,
                firmware_version: None,
                error_count: 0,
                recovery_attempts: 0,
                max_recovery_attempts: 3,
            }
        }

        fn state(&self) -> PanelState {
            self.state
        }

        fn check_firmware(&mut self, major: u8, minor: u8, patch: u8) -> bool {
            self.firmware_version = Some((major, minor, patch));
            // Require at least v1.0.0
            major >= 1
        }

        fn transition_to_ready(&mut self) -> bool {
            if self.state == PanelState::Init && self.firmware_version.is_some() {
                self.state = PanelState::Ready;
                true
            } else {
                false
            }
        }

        fn activate(&mut self) -> bool {
            if self.state == PanelState::Ready {
                self.state = PanelState::Active;
                true
            } else {
                false
            }
        }

        fn report_error(&mut self) {
            self.error_count += 1;
            self.state = PanelState::Error;
        }

        fn attempt_recovery(&mut self) -> bool {
            if self.state != PanelState::Error {
                return false;
            }
            self.recovery_attempts += 1;
            if self.recovery_attempts <= self.max_recovery_attempts {
                self.state = PanelState::Recovery;
                true
            } else {
                false
            }
        }

        fn complete_recovery(&mut self) -> bool {
            if self.state == PanelState::Recovery {
                self.state = PanelState::Ready;
                self.recovery_attempts = 0;
                true
            } else {
                false
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 1. Panel registration (6 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_register_panel() {
        let mut reg = PanelRegistry::new();
        assert!(reg.register("/dev/hidraw0".into(), Box::new(radio_panel())));
        assert_eq!(reg.count(), 1);
        let p = reg.get("/dev/hidraw0").unwrap();
        assert_eq!(p.name(), "Saitek Radio Panel");
        assert_eq!(p.vendor_id(), 0x06A3);
        assert_eq!(p.product_id(), 0x0D05);
    }

    #[test]
    fn depth_unregister_panel() {
        let mut reg = PanelRegistry::new();
        reg.register("/dev/hidraw0".into(), Box::new(radio_panel()));
        assert!(reg.unregister("/dev/hidraw0"));
        assert_eq!(reg.count(), 0);
        assert!(reg.get("/dev/hidraw0").is_none());
        // Unregister again returns false
        assert!(!reg.unregister("/dev/hidraw0"));
    }

    #[test]
    fn depth_panel_discovery() {
        let mut reg = PanelRegistry::new();
        reg.register("/dev/hidraw0".into(), Box::new(radio_panel()));
        reg.register("/dev/hidraw1".into(), Box::new(multi_panel()));
        reg.register("/dev/hidraw2".into(), Box::new(switch_panel()));

        let mut discovered = reg.discover();
        discovered.sort();
        assert_eq!(discovered.len(), 3);
        assert!(discovered.contains(&"/dev/hidraw0"));
        assert!(discovered.contains(&"/dev/hidraw1"));
        assert!(discovered.contains(&"/dev/hidraw2"));
    }

    #[test]
    fn depth_panel_capabilities_query() {
        let mut reg = PanelRegistry::new();
        reg.register("/dev/hidraw0".into(), Box::new(multi_panel()));

        let (name, leds, report_size) = reg.capabilities("/dev/hidraw0").unwrap();
        assert_eq!(name, "Saitek Multi Panel");
        assert_eq!(leds.len(), 8);
        assert!(leds.contains(&"AP"));
        assert!(leds.contains(&"HDG"));
        assert_eq!(report_size, 13);

        // Non-existent panel
        assert!(reg.capabilities("/dev/hidraw99").is_none());
    }

    #[test]
    fn depth_multiple_panel_types() {
        let mut reg = PanelRegistry::new();
        reg.register("radio".into(), Box::new(radio_panel()));
        reg.register("multi".into(), Box::new(multi_panel()));
        reg.register("switch".into(), Box::new(switch_panel()));

        // Each panel type has different LED sets and report sizes
        let radio = reg.get("radio").unwrap();
        let multi = reg.get("multi").unwrap();
        let switch = reg.get("switch").unwrap();

        assert_eq!(radio.led_names().len(), 4);
        assert_eq!(multi.led_names().len(), 8);
        assert_eq!(switch.led_names().len(), 3);

        assert_ne!(radio.product_id(), multi.product_id());
        assert_ne!(multi.product_id(), switch.product_id());
        assert_ne!(radio.output_report_size(), switch.output_report_size());
    }

    #[test]
    fn depth_panel_hot_plug() {
        let mut reg = PanelRegistry::new();

        // Simulate connect
        assert!(reg.register("/dev/hidraw0".into(), Box::new(radio_panel())));
        assert_eq!(reg.count(), 1);

        // Simulate disconnect
        assert!(reg.unregister("/dev/hidraw0"));
        assert_eq!(reg.count(), 0);

        // Simulate reconnect on different path
        assert!(reg.register("/dev/hidraw3".into(), Box::new(radio_panel())));
        assert_eq!(reg.count(), 1);
        assert_eq!(
            reg.get("/dev/hidraw3").unwrap().name(),
            "Saitek Radio Panel"
        );

        // Duplicate registration is rejected
        assert!(!reg.register("/dev/hidraw3".into(), Box::new(radio_panel())));
        assert_eq!(reg.count(), 1);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 2. Display protocol (6 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_seven_segment_data() {
        // All standard display formatters produce 5-char strings
        assert_eq!(format_heading(0), "    0");
        assert_eq!(format_heading(90), "   90");
        assert_eq!(format_heading(359), "  359");
        assert_eq!(format_heading(360), "    0"); // wrap

        assert_eq!(format_altitude(0), "    0");
        assert_eq!(format_altitude(35000), "35000");
        assert_eq!(format_altitude(-200), " -200");

        // Verify all outputs are exactly 5 chars (7-segment display width)
        for deg in (0..=720).step_by(15) {
            assert_eq!(format_heading(deg).len(), 5, "heading {deg}");
        }
    }

    #[test]
    fn depth_lcd_character_data() {
        // COM/NAV frequencies simulate LCD character display
        assert_eq!(format_com_freq(118_000), "11800");
        assert_eq!(format_com_freq(121_500), "12150"); // guard
        assert_eq!(format_com_freq(136_975), "13697");

        assert_eq!(format_nav_freq(108_000), "10800");
        assert_eq!(format_nav_freq(110_300), "11030"); // ILS
        assert_eq!(format_nav_freq(117_950), "11795");

        // ADF and XPDR
        assert_eq!(format_adf(340), "  340");
        assert_eq!(format_xpdr(1200), " 1200");
        assert_eq!(format_xpdr(7700), " 7700");
        assert_eq!(format_xpdr(0), " 0000");
    }

    #[test]
    fn depth_led_on_off_blink() {
        let mut ctrl = LedController::new();
        ctrl.set_min_interval(Duration::ZERO);
        let target = LedTarget::Panel("GEAR".to_string());

        // On
        ctrl.execute_actions(&[Action::LedOn {
            target: "GEAR".to_string(),
        }])
        .unwrap();
        assert!(ctrl.get_led_state(&target).unwrap().on);
        assert!(ctrl.get_led_state(&target).unwrap().blink_rate.is_none());

        // Off
        ctrl.execute_actions(&[Action::LedOff {
            target: "GEAR".to_string(),
        }])
        .unwrap();
        assert!(!ctrl.get_led_state(&target).unwrap().on);

        // Blink
        ctrl.execute_actions(&[Action::LedBlink {
            target: "GEAR".to_string(),
            rate_hz: 4.0,
        }])
        .unwrap();
        assert_eq!(ctrl.get_led_state(&target).unwrap().blink_rate, Some(4.0));
    }

    #[test]
    fn depth_multi_color_led() {
        // Simulate multi-color by controlling separate R/G/B targets
        let mut ctrl = LedController::new();
        ctrl.set_min_interval(Duration::ZERO);

        // Green only (gear safe)
        ctrl.execute_actions(&[
            Action::LedOn {
                target: "GEAR_GREEN".to_string(),
            },
            Action::LedOff {
                target: "GEAR_RED".to_string(),
            },
        ])
        .unwrap();

        let green = LedTarget::Panel("GEAR_GREEN".to_string());
        let red = LedTarget::Panel("GEAR_RED".to_string());
        assert!(ctrl.get_led_state(&green).unwrap().on);
        assert!(!ctrl.get_led_state(&red).unwrap().on);

        // Amber (both on, different brightness)
        ctrl.execute_actions(&[
            Action::LedBrightness {
                target: "GEAR_GREEN".to_string(),
                brightness: 0.5,
            },
            Action::LedOn {
                target: "GEAR_RED".to_string(),
            },
            Action::LedBrightness {
                target: "GEAR_RED".to_string(),
                brightness: 1.0,
            },
        ])
        .unwrap();
        assert_eq!(ctrl.get_led_state(&green).unwrap().brightness, 0.5);
        assert!(ctrl.get_led_state(&red).unwrap().on);
        assert_eq!(ctrl.get_led_state(&red).unwrap().brightness, 1.0);
    }

    #[test]
    fn depth_display_refresh_rate() {
        // Verify VS formatting across range (simulates rapid refresh)
        let mut all_five = true;
        for fpm in (-2000..=2000).step_by(100) {
            let s = format_vs(fpm);
            if s.len() != 5 {
                all_five = false;
            }
        }
        assert!(all_five, "all VS values must be 5 chars");

        // Edge clamping
        assert_eq!(format_vs(99999), " 9999");
        assert_eq!(format_vs(-99999), "-9999");
    }

    #[test]
    fn depth_display_buffering() {
        // PanelMessage::WriteDisplay buffers 5 bytes per row
        let msg0 = PanelMessage::WriteDisplay {
            row: 0,
            text: [0x3F, 0x06, 0x5B, 0x4F, 0x66], // "01234"
        };
        let msg1 = PanelMessage::WriteDisplay {
            row: 1,
            text: [0x6D, 0x7D, 0x07, 0x7F, 0x6F], // "56789"
        };

        if let PanelMessage::WriteDisplay { row, text } = &msg0 {
            assert_eq!(*row, 0);
            assert_eq!(text.len(), 5);
        }
        if let PanelMessage::WriteDisplay { row, text } = &msg1 {
            assert_eq!(*row, 1);
            assert_eq!(text[0], 0x6D);
        }

        // Clone round-trip preserves content
        assert_eq!(msg0, msg0.clone());
        assert_ne!(msg0, msg1);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 3. Input protocol (6 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_switch_state_read() {
        let event_on = PanelEvent::SwitchChange {
            name: "MASTER_BAT",
            on: true,
        };
        let event_off = PanelEvent::SwitchChange {
            name: "MASTER_BAT",
            on: false,
        };
        assert_ne!(event_on, event_off);

        // Switch events from panel parse_input
        let panel = switch_panel();
        let events = panel.parse_input(&[0x01, 0x00]).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], PanelEvent::ButtonPress { name: "MASTER_BAT" });

        let events = panel.parse_input(&[0x02, 0x00]).unwrap();
        assert_eq!(events[0], PanelEvent::ButtonPress { name: "MASTER_ALT" });
    }

    #[test]
    fn depth_encoder_rotation() {
        let cw = PanelEvent::EncoderTick {
            name: "OUTER",
            delta: 1,
        };
        let ccw = PanelEvent::EncoderTick {
            name: "OUTER",
            delta: -1,
        };
        let fast_cw = PanelEvent::EncoderTick {
            name: "OUTER",
            delta: 4,
        };

        assert_ne!(cw, ccw);
        assert_ne!(cw, fast_cw);

        // Delta sign indicates direction
        if let PanelEvent::EncoderTick { delta, .. } = cw {
            assert!(delta > 0);
        }
        if let PanelEvent::EncoderTick { delta, .. } = ccw {
            assert!(delta < 0);
        }
        if let PanelEvent::EncoderTick { delta, .. } = fast_cw {
            assert_eq!(delta, 4);
        }
    }

    #[test]
    fn depth_button_press_release() {
        let press = PanelEvent::ButtonPress { name: "AP" };
        let release = PanelEvent::ButtonRelease { name: "AP" };
        assert_ne!(press, release);

        // Different buttons
        let press2 = PanelEvent::ButtonPress { name: "HDG" };
        assert_ne!(press, press2);

        // Panel produces press events from report data
        let panel = multi_panel();
        let report = [0x00, 0x07]; // AP + HDG + ALT bits set
        let events = panel.parse_input(&report).unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn depth_hat_switch() {
        // Hat switch represented as selector with 8 positions
        let positions = [
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 0,
            }, // N
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 1,
            }, // NE
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 2,
            }, // E
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 3,
            }, // SE
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 4,
            }, // S
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 5,
            }, // SW
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 6,
            }, // W
            PanelEvent::SelectorChange {
                name: "HAT",
                position: 7,
            }, // NW
        ];

        for (i, pos) in positions.iter().enumerate() {
            if let PanelEvent::SelectorChange { position, .. } = pos {
                assert_eq!(*position, i as u8);
            }
        }
        // All distinct
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                assert_ne!(positions[i], positions[j]);
            }
        }
    }

    #[test]
    fn depth_analog_input() {
        // Analog input represented as selector with fine positions (0..255)
        let low = PanelEvent::SelectorChange {
            name: "THROTTLE",
            position: 0,
        };
        let mid = PanelEvent::SelectorChange {
            name: "THROTTLE",
            position: 128,
        };
        let high = PanelEvent::SelectorChange {
            name: "THROTTLE",
            position: 255,
        };
        assert_ne!(low, mid);
        assert_ne!(mid, high);
        assert_ne!(low, high);

        if let PanelEvent::SelectorChange { position, .. } = mid {
            assert_eq!(position, 128);
        }
    }

    #[test]
    fn depth_debounce() {
        // Debounce: identical consecutive events should be filtered
        let panel = switch_panel();

        let report_pressed = [0x01, 0x00]; // MASTER_BAT on
        let report_idle = [0x00, 0x00]; // nothing

        let events1 = panel.parse_input(&report_pressed).unwrap();
        let events2 = panel.parse_input(&report_pressed).unwrap();
        let events3 = panel.parse_input(&report_idle).unwrap();

        // Both reports produce same event — caller should debounce
        assert_eq!(events1, events2);
        // Idle produces no events
        assert!(events3.is_empty());

        // Simulate software debounce by comparing consecutive frames
        let mut last_events: Vec<PanelEvent> = Vec::new();
        let mut debounced_count = 0;
        for report in &[report_pressed, report_pressed, report_idle, report_pressed] {
            let current = panel.parse_input(report).unwrap();
            if current != last_events {
                debounced_count += 1;
                last_events = current;
            }
        }
        // pressed → (dup skipped) → idle → pressed = 3 transitions
        assert_eq!(debounced_count, 3);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 4. Variable binding (5 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_sim_variable_to_display() {
        // Sim variable → display formatter → 5-char output
        type DisplayFn = Box<dyn Fn(f64) -> String>;
        let bindings: Vec<(&str, DisplayFn)> = vec![
            (
                "heading_mag",
                Box::new(|v| format_heading(v as u16)),
            ),
            (
                "altitude_ft",
                Box::new(|v| format_altitude(v as i32)),
            ),
            (
                "vs_fpm",
                Box::new(|v| format_vs(v as i32)),
            ),
            (
                "com1_freq_khz",
                Box::new(|v| format_com_freq(v as u32)),
            ),
        ];

        let sim_values: HashMap<&str, f64> = [
            ("heading_mag", 270.0),
            ("altitude_ft", 35000.0),
            ("vs_fpm", -800.0),
            ("com1_freq_khz", 121_500.0),
        ]
        .into();

        for (var, formatter) in &bindings {
            let value = sim_values[var];
            let display = formatter(value);
            assert_eq!(display.len(), 5, "display for {var} should be 5 chars");
        }
    }

    #[test]
    fn depth_sim_variable_to_led() {
        // Sim variable threshold → LED action via rules evaluator
        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(Duration::ZERO);

        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR_GREEN').on()".to_string(),
                action: "led.panel('GEAR_GREEN').on()".to_string(),
            }],
            defaults: None,
        };
        let compiled = schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        let mut tel = HashMap::new();
        tel.insert("gear_down".to_string(), 1.0_f32);
        let actions = evaluator.evaluate(&compiled, &tel);
        assert_eq!(actions.len(), 1);
        assert!(matches!(&actions[0], Action::LedOn { target } if target == "GEAR_GREEN"));

        tel.insert("gear_down".to_string(), 0.0);
        let actions = evaluator.evaluate(&compiled, &tel);
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn depth_button_to_sim_command() {
        // Button press → lookup → sim command string
        let command_map: HashMap<&str, &str> = [
            ("AP_BTN", "AUTOPILOT_ON"),
            ("HDG_BTN", "HEADING_BUG_SELECT"),
            ("ALT_BTN", "ALTITUDE_HOLD"),
        ]
        .into();

        let panel = multi_panel();
        let report = [0x00, 0x01]; // AP_BTN pressed
        let events = panel.parse_input(&report).unwrap();
        assert_eq!(events.len(), 1);

        if let PanelEvent::ButtonPress { name } = &events[0] {
            let cmd = command_map.get(name).expect("button mapped to command");
            assert_eq!(*cmd, "AUTOPILOT_ON");
        } else {
            panic!("expected ButtonPress");
        }
    }

    #[test]
    fn depth_encoder_to_sim_variable() {
        // Encoder delta → sim variable increment
        let mut heading_bug: i32 = 270;
        let increments = [1, 1, -1, 4, -2]; // series of encoder ticks

        for delta in increments {
            heading_bug = (heading_bug + delta).rem_euclid(360);
        }
        // 270 + 1 + 1 - 1 + 4 - 2 = 273
        assert_eq!(heading_bug, 273);

        // Verify display formatting after encoder updates
        assert_eq!(format_heading(heading_bug as u16), "  273");
    }

    #[test]
    fn depth_bidirectional_binding() {
        // Sim → LED (display) AND button → sim (input) working together
        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(Duration::ZERO);

        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "ap_engaged".to_string(),
                    do_action: "led.panel('AP').on()".to_string(),
                    action: "led.panel('AP').on()".to_string(),
                },
                Rule {
                    when: "!ap_engaged".to_string(),
                    do_action: "led.panel('AP').off()".to_string(),
                    action: "led.panel('AP').off()".to_string(),
                },
            ],
            defaults: None,
        };
        let compiled = schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        let mut ctrl = LedController::new();
        ctrl.set_min_interval(Duration::ZERO);

        // Sim says AP engaged → LED on
        let mut tel = HashMap::new();
        tel.insert("ap_engaged".to_string(), 1.0_f32);
        let actions = evaluator.evaluate(&compiled, &tel);
        ctrl.execute_actions(actions).unwrap();
        assert!(
            ctrl.get_led_state(&LedTarget::Panel("AP".to_string()))
                .unwrap()
                .on
        );

        // User presses AP button → sim toggles AP off → LED off
        tel.insert("ap_engaged".to_string(), 0.0);
        let actions = evaluator.evaluate(&compiled, &tel);
        ctrl.execute_actions(actions).unwrap();
        assert!(
            !ctrl
                .get_led_state(&LedTarget::Panel("AP".to_string()))
                .unwrap()
                .on
        );
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 5. State machine (5 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_panel_lifecycle_states() {
        let mut sm = PanelStateMachine::new();
        assert_eq!(sm.state(), PanelState::Init);

        // Init → Ready requires firmware check
        assert!(!sm.transition_to_ready()); // no firmware yet
        assert!(sm.check_firmware(1, 2, 0));
        assert!(sm.transition_to_ready());
        assert_eq!(sm.state(), PanelState::Ready);

        // Ready → Active
        assert!(sm.activate());
        assert_eq!(sm.state(), PanelState::Active);

        // Active → Error
        sm.report_error();
        assert_eq!(sm.state(), PanelState::Error);

        // Error → Recovery → Ready
        assert!(sm.attempt_recovery());
        assert_eq!(sm.state(), PanelState::Recovery);
        assert!(sm.complete_recovery());
        assert_eq!(sm.state(), PanelState::Ready);
    }

    #[test]
    fn depth_invalid_state_transitions() {
        let mut sm = PanelStateMachine::new();

        // Cannot activate from Init
        assert!(!sm.activate());
        assert_eq!(sm.state(), PanelState::Init);

        // Cannot recover from Init
        assert!(!sm.attempt_recovery());

        // Cannot complete recovery from Init
        assert!(!sm.complete_recovery());

        // Set up properly
        sm.check_firmware(1, 0, 0);
        sm.transition_to_ready();

        // Cannot transition to Ready again
        assert!(!sm.transition_to_ready());

        // Activate then error
        sm.activate();
        sm.report_error();

        // Cannot activate from Error directly
        assert!(!sm.activate());
    }

    #[test]
    fn depth_error_recovery_cycle() {
        let mut sm = PanelStateMachine::new();
        sm.check_firmware(2, 0, 0);
        sm.transition_to_ready();
        sm.activate();

        // Multiple error/recovery cycles
        for _ in 0..3 {
            sm.report_error();
            assert_eq!(sm.state(), PanelState::Error);
            assert!(sm.attempt_recovery());
            assert!(sm.complete_recovery());
            assert_eq!(sm.state(), PanelState::Ready);
            sm.activate();
            assert_eq!(sm.state(), PanelState::Active);
        }
    }

    #[test]
    fn depth_recovery_attempt_limit() {
        let mut sm = PanelStateMachine::new();
        sm.check_firmware(1, 0, 0);
        sm.transition_to_ready();
        sm.activate();

        // Exhaust recovery attempts without completing recovery
        sm.report_error();
        assert!(sm.attempt_recovery()); // 1
        sm.state = PanelState::Error; // force back to error without completing
        assert!(sm.attempt_recovery()); // 2
        sm.state = PanelState::Error;
        assert!(sm.attempt_recovery()); // 3
        sm.state = PanelState::Error;
        assert!(!sm.attempt_recovery()); // 4 — exceeded max (3)
        assert_eq!(sm.state(), PanelState::Error); // stuck in Error
    }

    #[test]
    fn depth_firmware_version_check() {
        let mut sm = PanelStateMachine::new();

        // Old firmware fails
        assert!(!sm.check_firmware(0, 9, 9));
        // Cannot proceed
        assert!(sm.firmware_version.is_some());
        // But transition still works since firmware_version is set (check is advisory)
        // The caller should gate on the return value
        assert!(sm.transition_to_ready()); // firmware_version is Some

        // New state machine with valid firmware
        let mut sm2 = PanelStateMachine::new();
        assert!(sm2.check_firmware(1, 0, 0));
        assert!(sm2.transition_to_ready());
        assert_eq!(sm2.state(), PanelState::Ready);

        assert!(sm2.check_firmware(2, 5, 3));
        assert_eq!(sm2.firmware_version, Some((2, 5, 3)));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 6. Integration (5 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_panel_bus_sim_roundtrip() {
        // Panel event → evaluator → LED controller → backend write
        let (backend, writes) = RecordingBackend::new();
        let mut ctrl = LedController::with_backend(Box::new(backend));
        ctrl.set_min_interval(Duration::ZERO);

        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(Duration::ZERO);

        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR_N').on()".to_string(),
                action: "led.panel('GEAR_N').on()".to_string(),
            }],
            defaults: None,
        };
        let compiled = schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        // Simulate: panel reports gear-down switch → telemetry update → LED
        let mut tel = HashMap::new();
        tel.insert("gear_down".to_string(), 1.0_f32);

        let actions = evaluator.evaluate(&compiled, &tel);
        ctrl.execute_actions(actions).unwrap();

        let w = writes.lock().unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].0, LedTarget::Panel("GEAR_N".to_string()));
        assert!(w[0].1); // on = true
    }

    #[test]
    fn depth_profile_driven_panel_config() {
        // Load different rule profiles and verify different LED behavior
        let profiles = vec![
            (
                "VFR day",
                vec![Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('GEAR').on()".to_string(),
                    action: "led.panel('GEAR').on()".to_string(),
                }],
            ),
            (
                "IFR night",
                vec![
                    Rule {
                        when: "gear_down".to_string(),
                        do_action: "led.panel('GEAR').on()".to_string(),
                        action: "led.panel('GEAR').on()".to_string(),
                    },
                    Rule {
                        when: "ias > 200".to_string(),
                        do_action: "led.panel('OVERSPEED').on()".to_string(),
                        action: "led.panel('OVERSPEED').on()".to_string(),
                    },
                ],
            ),
        ];

        let mut tel = HashMap::new();
        tel.insert("gear_down".to_string(), 1.0_f32);
        tel.insert("ias".to_string(), 250.0_f32);

        for (profile_name, rules) in profiles {
            let mut evaluator = RulesEvaluator::new();
            evaluator.set_min_eval_interval(Duration::ZERO);

            let schema = RulesSchema {
                schema: "flight.ledmap/1".to_string(),
                rules,
                defaults: None,
            };
            let compiled = schema.compile().unwrap();
            evaluator.initialize_for_program(&compiled.bytecode);

            let actions = evaluator.evaluate(&compiled, &tel);
            match profile_name {
                "VFR day" => assert_eq!(actions.len(), 1, "VFR should have 1 action"),
                "IFR night" => assert_eq!(actions.len(), 2, "IFR should have 2 actions"),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn depth_multi_panel_coordination() {
        // Multiple panels share the same LED controller and evaluator
        let mut ctrl = LedController::new();
        ctrl.set_min_interval(Duration::ZERO);

        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(Duration::ZERO);

        // Rules target LEDs on different panels
        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('SWITCH_GEAR_N').on()".to_string(),
                    action: "led.panel('SWITCH_GEAR_N').on()".to_string(),
                },
                Rule {
                    when: "gear_down".to_string(),
                    do_action: "led.panel('MULTI_AP').on()".to_string(),
                    action: "led.panel('MULTI_AP').on()".to_string(),
                },
            ],
            defaults: None,
        };
        let compiled = schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        let mut tel = HashMap::new();
        tel.insert("gear_down".to_string(), 1.0_f32);

        let actions = evaluator.evaluate(&compiled, &tel);
        ctrl.execute_actions(actions).unwrap();

        // Both panels' LEDs should be updated
        assert!(
            ctrl.get_led_state(&LedTarget::Panel("SWITCH_GEAR_N".to_string()))
                .unwrap()
                .on
        );
        assert!(
            ctrl.get_led_state(&LedTarget::Panel("MULTI_AP".to_string()))
                .unwrap()
                .on
        );
    }

    #[test]
    fn depth_codec_roundtrip_all_messages() {
        // Test PanelCodec encode→decode for every message type using a mock
        struct TestCodec;
        impl PanelCodec for TestCodec {
            fn encode(&self, msg: &PanelMessage) -> Option<Vec<u8>> {
                match msg {
                    PanelMessage::ReadState => Some(vec![0x01]),
                    PanelMessage::WriteDisplay { row, text } => {
                        let mut buf = vec![0x02, *row];
                        buf.extend_from_slice(text);
                        Some(buf)
                    }
                    PanelMessage::SetLed { led_index, on } => {
                        Some(vec![0x03, *led_index, u8::from(*on)])
                    }
                    PanelMessage::SetBacklight { brightness } => Some(vec![0x04, *brightness]),
                    PanelMessage::Calibrate => Some(vec![0x05]),
                }
            }
            fn decode(&self, data: &[u8]) -> Option<PanelResponse> {
                match data.first()? {
                    0x80 => Some(PanelResponse::StateData {
                        data: data[1..].to_vec(),
                    }),
                    0x81 => Some(PanelResponse::Ack),
                    0xFF => Some(PanelResponse::Error {
                        code: *data.get(1).unwrap_or(&0) as u16,
                        message: "err".to_string(),
                    }),
                    _ => None,
                }
            }
        }

        let codec = TestCodec;

        // ReadState
        let enc = codec.encode(&PanelMessage::ReadState).unwrap();
        assert_eq!(enc, [0x01]);

        // WriteDisplay
        let text = [0x3F, 0x06, 0x5B, 0x4F, 0x66];
        let enc = codec
            .encode(&PanelMessage::WriteDisplay { row: 0, text })
            .unwrap();
        assert_eq!(enc.len(), 7);
        assert_eq!(enc[0], 0x02);
        assert_eq!(&enc[2..7], &text);

        // SetLed
        let enc = codec
            .encode(&PanelMessage::SetLed {
                led_index: 5,
                on: true,
            })
            .unwrap();
        assert_eq!(enc, [0x03, 5, 1]);

        // SetBacklight
        let enc = codec
            .encode(&PanelMessage::SetBacklight { brightness: 200 })
            .unwrap();
        assert_eq!(enc, [0x04, 200]);

        // Calibrate
        let enc = codec.encode(&PanelMessage::Calibrate).unwrap();
        assert_eq!(enc, [0x05]);

        // Decode responses
        assert_eq!(
            codec.decode(&[0x80, 0xAA]).unwrap(),
            PanelResponse::StateData {
                data: vec![0xAA]
            }
        );
        assert_eq!(codec.decode(&[0x81]).unwrap(), PanelResponse::Ack);
        assert!(matches!(
            codec.decode(&[0xFF, 0x02]).unwrap(),
            PanelResponse::Error { code: 2, .. }
        ));
    }

    #[test]
    fn depth_connection_protocol_sequence() {
        // Full connection protocol: connect → read state → set LEDs → calibrate → verify
        struct SequenceConnection {
            responses: Vec<PanelResponse>,
            sent: Vec<PanelMessage>,
        }

        impl PanelConnection for SequenceConnection {
            fn send(&mut self, msg: &PanelMessage) -> std::result::Result<(), String> {
                self.sent.push(msg.clone());
                // Queue appropriate response
                match msg {
                    PanelMessage::ReadState => {
                        self.responses.push(PanelResponse::StateData {
                            data: vec![0x00, 0x00, 0x00],
                        });
                    }
                    PanelMessage::SetLed { .. }
                    | PanelMessage::SetBacklight { .. }
                    | PanelMessage::WriteDisplay { .. } => {
                        self.responses.push(PanelResponse::Ack);
                    }
                    PanelMessage::Calibrate => {
                        self.responses.push(PanelResponse::Ack);
                    }
                }
                Ok(())
            }
            fn receive(&mut self, _timeout: Duration) -> Option<PanelResponse> {
                if self.responses.is_empty() {
                    None
                } else {
                    Some(self.responses.remove(0))
                }
            }
            fn poll(&self) -> bool {
                !self.responses.is_empty()
            }
        }

        let mut conn = SequenceConnection {
            responses: Vec::new(),
            sent: Vec::new(),
        };

        // Protocol sequence
        conn.send(&PanelMessage::ReadState).unwrap();
        let state = conn.receive(Duration::from_millis(100)).unwrap();
        assert!(matches!(state, PanelResponse::StateData { .. }));

        conn.send(&PanelMessage::SetLed {
            led_index: 0,
            on: true,
        })
        .unwrap();
        assert_eq!(
            conn.receive(Duration::from_millis(100)).unwrap(),
            PanelResponse::Ack
        );

        conn.send(&PanelMessage::WriteDisplay {
            row: 0,
            text: [0x3F, 0x06, 0x5B, 0x4F, 0x66],
        })
        .unwrap();
        assert_eq!(
            conn.receive(Duration::from_millis(100)).unwrap(),
            PanelResponse::Ack
        );

        conn.send(&PanelMessage::Calibrate).unwrap();
        assert_eq!(
            conn.receive(Duration::from_millis(100)).unwrap(),
            PanelResponse::Ack
        );

        // Verify all messages were sent
        assert_eq!(conn.sent.len(), 4);
        assert_eq!(conn.sent[0], PanelMessage::ReadState);
        assert!(matches!(conn.sent[1], PanelMessage::SetLed { .. }));
        assert!(matches!(conn.sent[2], PanelMessage::WriteDisplay { .. }));
        assert_eq!(conn.sent[3], PanelMessage::Calibrate);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 7. LED backend & latency (4 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_recording_backend_captures_all_writes() {
        let (backend, writes) = RecordingBackend::new();
        let mut ctrl = LedController::with_backend(Box::new(backend));
        ctrl.set_min_interval(Duration::ZERO);

        let actions = vec![
            Action::LedOn {
                target: "A".to_string(),
            },
            Action::LedBrightness {
                target: "B".to_string(),
                brightness: 0.7,
            },
            Action::LedOff {
                target: "C".to_string(),
            },
        ];
        ctrl.execute_actions(&actions).unwrap();

        let w = writes.lock().unwrap();
        assert_eq!(w.len(), 3);
        assert_eq!(w[0].0, LedTarget::Panel("A".to_string()));
        assert!(w[0].1); // on
        assert_eq!(w[1].0, LedTarget::Panel("B".to_string()));
        assert!((w[1].2 - 0.7).abs() < f32::EPSILON);
        assert_eq!(w[2].0, LedTarget::Panel("C".to_string()));
        assert!(!w[2].1); // off
    }

    #[test]
    fn depth_led_brightness_transitions() {
        let mut ctrl = LedController::new();
        ctrl.set_min_interval(Duration::ZERO);
        let target = LedTarget::Panel("DIM".to_string());

        let steps = [0.0, 0.25, 0.5, 0.75, 1.0, 0.5, 0.0];
        for &b in &steps {
            ctrl.execute_actions(&[Action::LedBrightness {
                target: "DIM".to_string(),
                brightness: b,
            }])
            .unwrap();
            let state = ctrl.get_led_state(&target).unwrap();
            assert!(
                (state.brightness - b).abs() < f32::EPSILON,
                "expected {b}, got {}",
                state.brightness
            );
        }
    }

    #[test]
    fn depth_latency_stats_clear() {
        let mut ctrl = LedController::new();
        ctrl.set_min_interval(Duration::ZERO);

        for i in 0..5 {
            ctrl.execute_actions(&[Action::LedOn {
                target: format!("S{i}"),
            }])
            .unwrap();
        }
        let stats = ctrl.get_latency_stats().unwrap();
        assert_eq!(stats.sample_count, 5);

        ctrl.clear_latency_stats();
        assert!(ctrl.get_latency_stats().is_none());

        // New samples after clear
        ctrl.execute_actions(&[Action::LedOn {
            target: "S_NEW".to_string(),
        }])
        .unwrap();
        let stats = ctrl.get_latency_stats().unwrap();
        assert_eq!(stats.sample_count, 1);
    }

    #[test]
    fn depth_led_controller_default_trait() {
        let ctrl = LedController::default();
        assert!(ctrl.get_led_state(&LedTarget::Indexer).is_none());
        assert!(ctrl.get_latency_stats().is_none());
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 8. PanelId identity & hashing (2 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_panel_id_as_hashmap_key() {
        use std::collections::HashMap;

        let id_a = PanelId {
            vendor_id: 0x06A3,
            product_id: 0x0D05,
            device_path: "/dev/hidraw0".to_string(),
        };
        let id_b = PanelId {
            vendor_id: 0x06A3,
            product_id: 0x0D06,
            device_path: "/dev/hidraw1".to_string(),
        };

        let mut map: HashMap<PanelId, &str> = HashMap::new();
        map.insert(id_a.clone(), "radio");
        map.insert(id_b.clone(), "multi");

        assert_eq!(map[&id_a], "radio");
        assert_eq!(map[&id_b], "multi");
        assert_eq!(map.len(), 2);

        // Same key overwrites
        map.insert(id_a.clone(), "radio_v2");
        assert_eq!(map[&id_a], "radio_v2");
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn depth_panel_id_display_format() {
        let id = PanelId {
            vendor_id: 0x0000,
            product_id: 0xFFFF,
            device_path: r"\\?\hid#vid_06a3".to_string(),
        };
        let s = id.to_string();
        assert!(s.contains("0000:FFFF"));
        assert!(s.contains(r"\\?\hid#vid_06a3"));
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 9. Display edge cases (2 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_com_nav_freq_clamping() {
        // Below-range COM clamps to lower bound
        assert_eq!(format_com_freq(100_000), "11800");
        // Above-range COM clamps to upper bound
        assert_eq!(format_com_freq(200_000), "13697");

        // Below-range NAV
        assert_eq!(format_nav_freq(50_000), "10800");
        // Above-range NAV
        assert_eq!(format_nav_freq(200_000), "11795");

        // ADF boundary clamping
        assert_eq!(format_adf(0), "  190");
        assert_eq!(format_adf(5000), " 1750");

        // XPDR clamping
        assert_eq!(format_xpdr(9999), " 7777");
    }

    #[test]
    fn depth_altitude_boundary_formatting() {
        // Exact boundaries
        assert_eq!(format_altitude(99999), "99999");
        assert_eq!(format_altitude(-9999), "-9999");
        // Just within range
        assert_eq!(format_altitude(1), "    1");
        assert_eq!(format_altitude(-1), "   -1");
        // Negative zero is still "    0"
        // (Rust i32 has no negative zero, so -0 == 0)
        assert_eq!(format_altitude(0), "    0");
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // 10. Evaluator edge cases (2 tests)
    // ═══════════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_evaluator_missing_telemetry_defaults_zero() {
        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(Duration::ZERO);

        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
            defaults: None,
        };
        let compiled = schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        // Empty telemetry — missing variable defaults to 0.0 (falsy)
        let tel: HashMap<String, f32> = HashMap::new();
        let actions = evaluator.evaluate(&compiled, &tel);
        assert_eq!(actions.len(), 0, "missing var should default to 0 (false)");
    }

    #[test]
    fn depth_evaluator_reuse_across_programs() {
        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(Duration::ZERO);

        // First program
        let schema1 = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
            defaults: None,
        };
        let compiled1 = schema1.compile().unwrap();
        evaluator.initialize_for_program(&compiled1.bytecode);

        let mut tel = HashMap::new();
        tel.insert("gear_down".to_string(), 1.0_f32);
        let actions = evaluator.evaluate(&compiled1, &tel);
        assert_eq!(actions.len(), 1);

        // Re-initialize with a different program
        let schema2 = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "flaps_deployed".to_string(),
                do_action: "led.panel('FLAPS').on()".to_string(),
                action: "led.panel('FLAPS').on()".to_string(),
            }],
            defaults: None,
        };
        let compiled2 = schema2.compile().unwrap();
        evaluator.initialize_for_program(&compiled2.bytecode);

        // Old telemetry variable is irrelevant to new program
        let actions = evaluator.evaluate(&compiled2, &tel);
        assert_eq!(actions.len(), 0, "old var should not trigger new program");

        tel.insert("flaps_deployed".to_string(), 1.0);
        let actions = evaluator.evaluate(&compiled2, &tel);
        assert_eq!(actions.len(), 1);
    }
}
