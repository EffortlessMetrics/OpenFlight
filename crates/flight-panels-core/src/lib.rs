// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Core panel rules evaluation and LED control.

pub mod evaluator;
pub mod led;

pub use evaluator::RulesEvaluator;
pub use led::{LatencyStats, LedController, LedState, LedTarget};

#[cfg(test)]
mod tests {
    use super::*;
    use flight_core::rules::Action;
    use std::time::Duration;

    /// Verify that LedTarget variants implement equality and hashing correctly.
    #[test]
    fn test_led_target_equality() {
        assert_eq!(
            LedTarget::Panel("GEAR".to_string()),
            LedTarget::Panel("GEAR".to_string())
        );
        assert_ne!(
            LedTarget::Panel("GEAR".to_string()),
            LedTarget::Panel("FLAPS".to_string())
        );
        assert_ne!(LedTarget::Indexer, LedTarget::Panel("GEAR".to_string()));
        assert_eq!(LedTarget::Indexer, LedTarget::Indexer);
        assert_eq!(
            LedTarget::Custom("x".to_string()),
            LedTarget::Custom("x".to_string())
        );
    }

    /// Verify that LED brightness is clamped to [0.0, 1.0].
    #[test]
    fn test_led_brightness_clamping() {
        let mut controller = LedController::new();
        controller.set_min_interval(Duration::ZERO);

        // Over-range brightness should be clamped to 1.0
        controller
            .execute_actions(&[Action::LedBrightness {
                target: "OVER".to_string(),
                brightness: 2.5,
            }])
            .unwrap();
        let state = controller
            .get_led_state(&LedTarget::Panel("OVER".to_string()))
            .unwrap();
        assert_eq!(state.brightness, 1.0, "brightness should be clamped to 1.0");

        // Under-range brightness should be clamped to 0.0
        controller
            .execute_actions(&[Action::LedBrightness {
                target: "UNDER".to_string(),
                brightness: -0.5,
            }])
            .unwrap();
        let state = controller
            .get_led_state(&LedTarget::Panel("UNDER".to_string()))
            .unwrap();
        assert_eq!(state.brightness, 0.0, "brightness should be clamped to 0.0");
    }

    /// Verify the on → off → blink state-transition sequence for a panel LED.
    #[test]
    fn test_led_on_off_state_transition() {
        let mut controller = LedController::new();
        controller.set_min_interval(Duration::ZERO);
        let target = LedTarget::Panel("MASTER_WARN".to_string());

        controller
            .execute_actions(&[Action::LedOn {
                target: "MASTER_WARN".to_string(),
            }])
            .unwrap();
        assert!(
            controller.get_led_state(&target).unwrap().on,
            "LED should be on"
        );

        controller
            .execute_actions(&[Action::LedOff {
                target: "MASTER_WARN".to_string(),
            }])
            .unwrap();
        assert!(
            !controller.get_led_state(&target).unwrap().on,
            "LED should be off"
        );

        controller
            .execute_actions(&[Action::LedBlink {
                target: "MASTER_WARN".to_string(),
                rate_hz: 2.0,
            }])
            .unwrap();
        assert_eq!(
            controller.get_led_state(&target).unwrap().blink_rate,
            Some(2.0),
            "LED should have blink rate"
        );
    }

    /// Verify that a freshly created LedController has no tracked LED state.
    #[test]
    fn test_led_controller_initial_state() {
        let controller = LedController::new();
        // No LEDs registered yet — querying any target returns None
        assert!(
            controller
                .get_led_state(&LedTarget::Panel("ANY".to_string()))
                .is_none()
        );
        assert!(controller.get_led_state(&LedTarget::Indexer).is_none());
        // No latency samples yet
        assert!(controller.get_latency_stats().is_none());
    }

    /// Verify that RulesEvaluator::new() produces a valid, usable evaluator.
    #[test]
    fn test_rules_evaluator_new_is_valid() {
        use flight_core::rules::{Rule, RulesSchema};
        use std::collections::HashMap;

        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(Duration::ZERO);

        let schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "flaps_deployed".to_string(),
                do_action: "led.panel('FLAPS').on()".to_string(),
                action: "led.panel('FLAPS').on()".to_string(),
            }],
            defaults: None,
        };
        let compiled = schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        let mut telemetry = HashMap::new();
        telemetry.insert("flaps_deployed".to_string(), 1.0_f32);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(
            actions.len(),
            1,
            "expected one action when condition is true"
        );

        telemetry.insert("flaps_deployed".to_string(), 0.0_f32);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(
            actions.len(),
            0,
            "expected no actions when condition is false"
        );
    }
}
