// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Button state and action management for StreamDeck devices
//!
//! Tracks press/release/hold state for every button and dispatches the
//! appropriate [`ButtonAction`] when state transitions occur. Hold detection
//! uses a configurable duration threshold.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Default hold threshold (500 ms).
const DEFAULT_HOLD_THRESHOLD: Duration = Duration::from_millis(500);

// ── Actions ──────────────────────────────────────────────────────────────────

/// An action bound to a StreamDeck button.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonAction {
    /// Fire a simulator command (e.g. `"AP_MASTER"`).
    SimCommand(String),
    /// Switch to a named profile.
    ProfileSwitch(String),
    /// Toggle a named state flag.
    ToggleState(String),
    /// Execute a sequence of actions in order.
    MacroSequence(Vec<ButtonAction>),
}

// ── Button configuration ─────────────────────────────────────────────────────

/// Static configuration for a single button slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonConfig {
    /// Text label rendered on the key face.
    pub label: String,
    /// Optional path to a custom icon image.
    pub icon_path: Option<String>,
    /// Action executed on press (or tap).
    pub action: ButtonAction,
}

// ── Display state ────────────────────────────────────────────────────────────

/// Visual state of a button, consumed by the renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonDisplayState {
    pub label: String,
    pub background_color: [u8; 3],
    pub text_color: [u8; 3],
    pub icon_path: Option<String>,
    pub active: bool,
}

impl Default for ButtonDisplayState {
    fn default() -> Self {
        Self {
            label: String::new(),
            background_color: [0x1A, 0x1A, 0x2E],
            text_color: [0xFF, 0xFF, 0xFF],
            icon_path: None,
            active: false,
        }
    }
}

// ── Internal per-button tracking ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PressPhase {
    Released,
    Pressed,
    Held,
}

#[derive(Debug)]
struct ButtonState {
    phase: PressPhase,
    press_start: Option<Instant>,
    toggle_on: bool,
}

impl ButtonState {
    fn new() -> Self {
        Self {
            phase: PressPhase::Released,
            press_start: None,
            toggle_on: false,
        }
    }
}

// ── Dispatched event ─────────────────────────────────────────────────────────

/// Events emitted by [`ButtonManager`] when button state changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ButtonEvent {
    Pressed { button_id: u8 },
    Released { button_id: u8 },
    Held { button_id: u8 },
    ActionDispatched { button_id: u8, action: ButtonAction },
}

// ── ButtonManager ────────────────────────────────────────────────────────────

/// Manages button state, hold detection, and action dispatch.
pub struct ButtonManager {
    button_count: u8,
    configs: HashMap<u8, ButtonConfig>,
    states: HashMap<u8, ButtonState>,
    hold_threshold: Duration,
    events: Vec<ButtonEvent>,
    toggled: HashMap<String, bool>,
}

impl ButtonManager {
    /// Create a new manager for a device with `button_count` keys.
    pub fn new(button_count: u8) -> Self {
        let mut states = HashMap::new();
        for id in 0..button_count {
            states.insert(id, ButtonState::new());
        }
        Self {
            button_count,
            configs: HashMap::new(),
            states,
            hold_threshold: DEFAULT_HOLD_THRESHOLD,
            events: Vec::new(),
            toggled: HashMap::new(),
        }
    }

    /// Override the hold-detection threshold.
    pub fn set_hold_threshold(&mut self, threshold: Duration) {
        self.hold_threshold = threshold;
    }

    /// Bind a [`ButtonConfig`] to a button slot.
    pub fn set_config(&mut self, button_id: u8, config: ButtonConfig) {
        self.configs.insert(button_id, config);
    }

    /// Process a button press event.
    pub fn handle_press(&mut self, button_id: u8) {
        if button_id >= self.button_count {
            return;
        }
        let state = self
            .states
            .entry(button_id)
            .or_insert_with(ButtonState::new);
        state.phase = PressPhase::Pressed;
        state.press_start = Some(Instant::now());
        self.events.push(ButtonEvent::Pressed { button_id });
    }

    /// Process a button release event.
    pub fn handle_release(&mut self, button_id: u8) {
        if button_id >= self.button_count {
            return;
        }
        let was_held = {
            let state = self
                .states
                .entry(button_id)
                .or_insert_with(ButtonState::new);
            let held = state.phase == PressPhase::Held;
            state.phase = PressPhase::Released;
            state.press_start = None;
            held
        };

        self.events.push(ButtonEvent::Released { button_id });

        // Dispatch action on release (tap) only when not held.
        if !was_held {
            if let Some(config) = self.configs.get(&button_id) {
                let action = config.action.clone();
                self.apply_action(button_id, &action);
            }
        }
    }

    /// Tick hold detection — call this periodically (e.g. each frame).
    pub fn tick(&mut self) {
        let now = Instant::now();
        let threshold = self.hold_threshold;
        let mut newly_held = Vec::new();

        for (&id, state) in &mut self.states {
            if state.phase == PressPhase::Pressed {
                if let Some(start) = state.press_start {
                    if now.duration_since(start) >= threshold {
                        state.phase = PressPhase::Held;
                        newly_held.push(id);
                    }
                }
            }
        }
        for id in newly_held {
            self.events.push(ButtonEvent::Held { button_id: id });
        }
    }

    /// Drain all pending events.
    pub fn drain_events(&mut self) -> Vec<ButtonEvent> {
        std::mem::take(&mut self.events)
    }

    /// Query the current display state for a button.
    pub fn get_display_state(&self, button_id: u8) -> ButtonDisplayState {
        let config = match self.configs.get(&button_id) {
            Some(c) => c,
            None => return ButtonDisplayState::default(),
        };
        let state = self.states.get(&button_id);
        let phase = state.map_or(PressPhase::Released, |s| s.phase);

        let active = self.is_active(button_id);

        let background_color = match phase {
            PressPhase::Pressed => [0x33, 0x33, 0x55],
            PressPhase::Held => [0x55, 0x33, 0x33],
            PressPhase::Released if active => [0x00, 0x44, 0x22],
            PressPhase::Released => [0x1A, 0x1A, 0x2E],
        };

        let text_color = if active {
            [0x00, 0xFF, 0x88]
        } else {
            [0xFF, 0xFF, 0xFF]
        };

        ButtonDisplayState {
            label: config.label.clone(),
            background_color,
            text_color,
            icon_path: config.icon_path.clone(),
            active,
        }
    }

    /// Whether a button is in the "active/on" toggle state.
    pub fn is_active(&self, button_id: u8) -> bool {
        self.states.get(&button_id).is_some_and(|s| s.toggle_on)
    }

    /// Number of buttons this manager handles.
    pub fn button_count(&self) -> u8 {
        self.button_count
    }

    /// Whether a toggle state flag is on.
    pub fn is_state_toggled(&self, name: &str) -> bool {
        self.toggled.get(name).copied().unwrap_or(false)
    }

    // ── Internal ─────────────────────────────────────────────────────

    fn apply_action(&mut self, button_id: u8, action: &ButtonAction) {
        match action {
            ButtonAction::SimCommand(_) => {
                // Toggle the button's on/off visual state.
                if let Some(state) = self.states.get_mut(&button_id) {
                    state.toggle_on = !state.toggle_on;
                }
            }
            ButtonAction::ProfileSwitch(_) => { /* handled externally */ }
            ButtonAction::ToggleState(name) => {
                let val = self.toggled.entry(name.clone()).or_insert(false);
                *val = !*val;
                if let Some(state) = self.states.get_mut(&button_id) {
                    state.toggle_on = *val;
                }
            }
            ButtonAction::MacroSequence(actions) => {
                for sub in actions {
                    self.apply_action(button_id, sub);
                }
            }
        }
        self.events.push(ButtonEvent::ActionDispatched {
            button_id,
            action: action.clone(),
        });
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_config(label: &str) -> ButtonConfig {
        ButtonConfig {
            label: label.to_string(),
            icon_path: None,
            action: ButtonAction::SimCommand("TEST_CMD".to_string()),
        }
    }

    // ── Press / release basics ─────────────────────────────────────

    #[test]
    fn test_press_emits_event() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("BTN0"));
        mgr.handle_press(0);
        let events = mgr.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ButtonEvent::Pressed { button_id: 0 }))
        );
    }

    #[test]
    fn test_release_emits_event() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("BTN0"));
        mgr.handle_press(0);
        mgr.drain_events();
        mgr.handle_release(0);
        let events = mgr.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ButtonEvent::Released { button_id: 0 }))
        );
    }

    #[test]
    fn test_release_dispatches_action() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("BTN0"));
        mgr.handle_press(0);
        mgr.handle_release(0);
        let events = mgr.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ButtonEvent::ActionDispatched { button_id: 0, .. }))
        );
    }

    #[test]
    fn test_out_of_range_button_ignored() {
        let mut mgr = ButtonManager::new(6);
        mgr.handle_press(10);
        mgr.handle_release(10);
        let events = mgr.drain_events();
        assert!(events.is_empty());
    }

    // ── Hold detection ─────────────────────────────────────────────

    #[test]
    fn test_hold_detection() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("BTN0"));
        mgr.set_hold_threshold(Duration::from_millis(0));

        mgr.handle_press(0);
        // Threshold is 0 ms so next tick should detect hold.
        mgr.tick();
        let events = mgr.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ButtonEvent::Held { button_id: 0 }))
        );
    }

    #[test]
    fn test_held_release_does_not_dispatch_action() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("BTN0"));
        mgr.set_hold_threshold(Duration::from_millis(0));

        mgr.handle_press(0);
        mgr.tick(); // triggers Held
        mgr.drain_events();

        mgr.handle_release(0);
        let events = mgr.drain_events();
        // Should have Released but NOT ActionDispatched.
        assert!(
            events
                .iter()
                .any(|e| matches!(e, ButtonEvent::Released { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, ButtonEvent::ActionDispatched { .. }))
        );
    }

    // ── Toggle state ───────────────────────────────────────────────

    #[test]
    fn test_sim_command_toggles_active() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("BTN0"));
        assert!(!mgr.is_active(0));

        mgr.handle_press(0);
        mgr.handle_release(0);
        assert!(mgr.is_active(0));

        mgr.handle_press(0);
        mgr.handle_release(0);
        assert!(!mgr.is_active(0));
    }

    #[test]
    fn test_toggle_state_action() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(
            0,
            ButtonConfig {
                label: "GR".to_string(),
                icon_path: None,
                action: ButtonAction::ToggleState("gear_down".to_string()),
            },
        );

        assert!(!mgr.is_state_toggled("gear_down"));
        mgr.handle_press(0);
        mgr.handle_release(0);
        assert!(mgr.is_state_toggled("gear_down"));

        mgr.handle_press(0);
        mgr.handle_release(0);
        assert!(!mgr.is_state_toggled("gear_down"));
    }

    // ── Macro sequence ─────────────────────────────────────────────

    #[test]
    fn test_macro_sequence_dispatches_all() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(
            0,
            ButtonConfig {
                label: "MACRO".to_string(),
                icon_path: None,
                action: ButtonAction::MacroSequence(vec![
                    ButtonAction::SimCommand("CMD_A".to_string()),
                    ButtonAction::SimCommand("CMD_B".to_string()),
                ]),
            },
        );

        mgr.handle_press(0);
        mgr.handle_release(0);
        let events = mgr.drain_events();
        // Inner sub-action dispatches + the macro dispatch itself.
        let dispatched: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, ButtonEvent::ActionDispatched { .. }))
            .collect();
        assert!(dispatched.len() >= 3);
    }

    // ── Display state ──────────────────────────────────────────────

    #[test]
    fn test_display_state_unconfigured() {
        let mgr = ButtonManager::new(6);
        let ds = mgr.get_display_state(0);
        assert_eq!(ds, ButtonDisplayState::default());
    }

    #[test]
    fn test_display_state_inactive() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("HDG"));
        let ds = mgr.get_display_state(0);
        assert_eq!(ds.label, "HDG");
        assert!(!ds.active);
        assert_eq!(ds.background_color, [0x1A, 0x1A, 0x2E]);
    }

    #[test]
    fn test_display_state_active() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("HDG"));
        mgr.handle_press(0);
        mgr.handle_release(0);
        let ds = mgr.get_display_state(0);
        assert!(ds.active);
        assert_eq!(ds.text_color, [0x00, 0xFF, 0x88]);
    }

    #[test]
    fn test_display_state_while_pressed() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("HDG"));
        mgr.handle_press(0);
        let ds = mgr.get_display_state(0);
        assert_eq!(ds.background_color, [0x33, 0x33, 0x55]);
    }

    #[test]
    fn test_display_state_while_held() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(0, simple_config("HDG"));
        mgr.set_hold_threshold(Duration::from_millis(0));
        mgr.handle_press(0);
        mgr.tick();
        let ds = mgr.get_display_state(0);
        assert_eq!(ds.background_color, [0x55, 0x33, 0x33]);
    }

    #[test]
    fn test_display_state_with_icon_path() {
        let mut mgr = ButtonManager::new(6);
        mgr.set_config(
            0,
            ButtonConfig {
                label: "ICN".to_string(),
                icon_path: Some("icons/test.png".to_string()),
                action: ButtonAction::SimCommand("X".to_string()),
            },
        );
        let ds = mgr.get_display_state(0);
        assert_eq!(ds.icon_path.as_deref(), Some("icons/test.png"));
    }

    // ── Multiple buttons ───────────────────────────────────────────

    #[test]
    fn test_multiple_buttons_independent() {
        let mut mgr = ButtonManager::new(15);
        mgr.set_config(0, simple_config("A"));
        mgr.set_config(5, simple_config("B"));

        mgr.handle_press(0);
        mgr.handle_release(0);

        assert!(mgr.is_active(0));
        assert!(!mgr.is_active(5));
    }

    #[test]
    fn test_button_count() {
        let mgr = ButtonManager::new(32);
        assert_eq!(mgr.button_count(), 32);
    }
}
