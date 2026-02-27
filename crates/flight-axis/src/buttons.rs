// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Button macro system.
//!
//! Maps physical button presses, chords, and holds to virtual actions with
//! zero allocations on the processing hot path.
//!
//! # Supported triggers
//! - [`MacroTrigger::OnPress`] — fires on the leading edge (button press)
//! - [`MacroTrigger::OnRelease`] — fires on the trailing edge (button release)
//! - [`MacroTrigger::OnHold`] — fires after a hold threshold, then repeats at
//!   a fixed interval while the chord remains held

/// A set of button indices that form a chord (up to 4 buttons, indices 0–63).
///
/// Chord buttons are stored in sorted order so that `[0, 1]` and `[1, 0]`
/// compare equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ButtonChord {
    /// Sorted button indices; unused slots are 0.
    buttons: [u8; 4],
    count: u8,
}

impl ButtonChord {
    /// Creates a single-button chord.
    pub fn single(btn: u8) -> Self {
        Self {
            buttons: [btn, 0, 0, 0],
            count: 1,
        }
    }

    /// Creates a chord from a slice of button indices (1–4 buttons).
    ///
    /// Returns an error if the slice is empty, contains more than 4 buttons,
    /// or contains duplicate indices.
    pub fn from_slice(buttons: &[u8]) -> Result<Self, ButtonError> {
        if buttons.is_empty() {
            return Err(ButtonError::EmptyChord);
        }
        if buttons.len() > 4 {
            return Err(ButtonError::TooManyButtons);
        }
        let mut arr = [0u8; 4];
        for (i, &b) in buttons.iter().enumerate() {
            arr[i] = b;
        }
        arr[..buttons.len()].sort_unstable();
        for i in 1..buttons.len() {
            if arr[i] == arr[i - 1] {
                return Err(ButtonError::DuplicateButton);
            }
        }
        Ok(Self {
            buttons: arr,
            count: buttons.len() as u8,
        })
    }

    /// Returns the button indices that form this chord (sorted).
    #[inline]
    pub fn buttons(&self) -> &[u8] {
        &self.buttons[..self.count as usize]
    }

    /// Returns the number of buttons in this chord.
    #[inline]
    pub fn len(&self) -> usize {
        self.count as usize
    }

    /// Returns `true` if the chord has no buttons.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Action emitted when a macro fires.
#[derive(Debug, Clone, PartialEq)]
pub enum MacroAction {
    /// Emit a virtual button press.
    VirtualButton { index: u8 },
    /// Add `delta` to an axis value.
    AxisOffset { axis_index: u8, delta: f32 },
    /// Set an axis to an exact value.
    AxisSet { axis_index: u8, value: f32 },
    /// Swallow the button press without emitting anything.
    Suppress,
}

impl Default for MacroAction {
    fn default() -> Self {
        Self::Suppress
    }
}

/// Specifies when a [`ButtonMacro`] fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroTrigger {
    /// Fires on the leading edge (button pressed this tick).
    OnPress,
    /// Fires on the trailing edge (button released this tick).
    OnRelease,
    /// Fires once after `hold_ticks` ticks, then every `interval_ticks` ticks
    /// while the chord remains fully held.
    OnHold {
        /// Minimum hold duration (in ticks) before the first fire.
        hold_ticks: u32,
        /// Repeat interval (in ticks) after the initial hold.
        interval_ticks: u32,
    },
}

/// A single button macro binding.
#[derive(Debug, Clone)]
pub struct ButtonMacro {
    pub chord: ButtonChord,
    pub trigger: MacroTrigger,
    pub action: MacroAction,
    pub enabled: bool,
}

/// Errors produced by the button macro subsystem.
#[derive(Debug)]
pub enum ButtonError {
    /// A chord contained more than 4 buttons.
    TooManyButtons,
    /// A chord contained the same button index more than once.
    DuplicateButton,
    /// An empty chord was supplied.
    EmptyChord,
    /// An axis index was out of range.
    InvalidAxisIndex,
}

/// Tracks button state and fires macros.
///
/// `ButtonProcessor` is designed for the RT hot path: `process` never
/// allocates, locks, or performs any blocking operations.
pub struct ButtonProcessor {
    /// Latest button state (bitmask, indices 0–63).
    state: u64,
    /// Button state from the previous tick.
    prev_state: u64,
    /// Per-button hold counters: `hold_timers[i]` is the number of consecutive
    /// ticks button `i` has been held (0 when not held).
    hold_timers: [u32; 64],
    /// Registered macros.  Registration happens off the hot path.
    macros: Vec<ButtonMacro>,
}

impl Default for ButtonProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl ButtonProcessor {
    /// Creates a new, empty `ButtonProcessor`.
    pub fn new() -> Self {
        Self {
            state: 0,
            prev_state: 0,
            hold_timers: [0u32; 64],
            macros: Vec::new(),
        }
    }

    /// Registers a macro.
    ///
    /// Registration always succeeds; the `Result` return type is reserved for
    /// future conflict detection.
    pub fn register_macro(&mut self, m: ButtonMacro) -> Result<(), ButtonError> {
        self.macros.push(m);
        Ok(())
    }

    /// Processes a button state update and fires any matching macros.
    ///
    /// Writes up to 8 triggered [`MacroAction`]s into `output` and returns the
    /// number of actions written.  Slots beyond the returned count are
    /// unspecified.
    ///
    /// # RT safety
    /// This method never allocates, locks, or blocks.
    pub fn process(&mut self, new_state: u64, output: &mut [MacroAction; 8]) -> usize {
        let rising = new_state & !self.prev_state;
        let falling = self.prev_state & !new_state;

        // Update per-button hold timers.
        for i in 0..64u8 {
            if new_state & (1u64 << i) != 0 {
                self.hold_timers[i as usize] = self.hold_timers[i as usize].saturating_add(1);
            } else {
                self.hold_timers[i as usize] = 0;
            }
        }

        let mut count = 0usize;

        for m in &self.macros {
            if !m.enabled || count >= 8 {
                continue;
            }
            let chord = m.chord.buttons();

            let fired = match m.trigger {
                MacroTrigger::OnPress => chord.iter().all(|&b| b < 64 && rising & (1u64 << b) != 0),

                MacroTrigger::OnRelease => {
                    chord.iter().all(|&b| b < 64 && falling & (1u64 << b) != 0)
                }

                MacroTrigger::OnHold {
                    hold_ticks,
                    interval_ticks,
                } => {
                    let all_held = chord
                        .iter()
                        .all(|&b| b < 64 && new_state & (1u64 << b) != 0);
                    if !all_held {
                        false
                    } else {
                        let min_timer = chord
                            .iter()
                            .map(|&b| self.hold_timers[b as usize])
                            .min()
                            .unwrap_or(0);
                        if min_timer >= hold_ticks {
                            let elapsed = min_timer - hold_ticks;
                            interval_ticks == 0 || elapsed % interval_ticks == 0
                        } else {
                            false
                        }
                    }
                }
            };

            if fired {
                output[count] = m.action.clone();
                count += 1;
            }
        }

        self.state = new_state;
        self.prev_state = new_state;

        count
    }

    /// Returns the number of registered macros.
    pub fn macro_count(&self) -> usize {
        self.macros.len()
    }

    /// Enables or disables the macro at the given index.
    ///
    /// Does nothing if the index is out of bounds.
    pub fn set_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(m) = self.macros.get_mut(index) {
            m.enabled = enabled;
        }
    }

    /// Removes all registered macros.
    pub fn clear(&mut self) {
        self.macros.clear();
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn make_output() -> [MacroAction; 8] {
        std::array::from_fn(|_| MacroAction::Suppress)
    }

    // ── Single-button press ───────────────────────────────────────────────────

    #[test]
    fn test_single_button_press_fires_macro() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 42 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        // Tick: button 0 goes from not-held to held.
        let n = proc.process(0b1, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], MacroAction::VirtualButton { index: 42 });
    }

    // ── Single-button release ─────────────────────────────────────────────────

    #[test]
    fn test_single_button_release_fires_macro() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(1),
            trigger: MacroTrigger::OnRelease,
            action: MacroAction::Suppress,
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        // Prime state: button 1 held.
        proc.process(0b10, &mut out);
        // Button 1 released.
        let n = proc.process(0b00, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], MacroAction::Suppress);
    }

    // ── Chord: all buttons required ───────────────────────────────────────────

    #[test]
    fn test_chord_requires_all_buttons() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::from_slice(&[0, 1]).unwrap(),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 7 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        // Both buttons pressed simultaneously.
        let n = proc.process(0b11, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], MacroAction::VirtualButton { index: 7 });
    }

    // ── Chord: partial press does not fire ────────────────────────────────────

    #[test]
    fn test_chord_partial_press_no_fire() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::from_slice(&[0, 1]).unwrap(),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 7 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        // Only button 0 pressed.
        let n = proc.process(0b01, &mut out);
        assert_eq!(n, 0);
    }

    // ── Hold: fires after threshold ───────────────────────────────────────────

    #[test]
    fn test_hold_fires_after_threshold() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnHold {
                hold_ticks: 3,
                interval_ticks: 1,
            },
            action: MacroAction::VirtualButton { index: 1 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        // Ticks 1 & 2: hold_timer < threshold.
        assert_eq!(proc.process(1, &mut out), 0);
        assert_eq!(proc.process(1, &mut out), 0);
        // Tick 3: hold_timer == threshold, elapsed == 0, fires.
        assert_eq!(proc.process(1, &mut out), 1);
        assert_eq!(out[0], MacroAction::VirtualButton { index: 1 });
    }

    // ── Hold: repeats at interval ─────────────────────────────────────────────

    #[test]
    fn test_hold_repeats_at_interval() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnHold {
                hold_ticks: 2,
                interval_ticks: 2,
            },
            action: MacroAction::VirtualButton { index: 5 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        proc.process(1, &mut out); // tick 1 — no fire
        assert_eq!(proc.process(1, &mut out), 1); // tick 2 — fire (elapsed=0)
        assert_eq!(proc.process(1, &mut out), 0); // tick 3 — elapsed=1, skip
        assert_eq!(proc.process(1, &mut out), 1); // tick 4 — elapsed=2, fire
    }

    // ── Hold: resets on release ───────────────────────────────────────────────

    #[test]
    fn test_hold_resets_on_release() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnHold {
                hold_ticks: 3,
                interval_ticks: 1,
            },
            action: MacroAction::VirtualButton { index: 2 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        // Hold to threshold.
        proc.process(1, &mut out);
        proc.process(1, &mut out);
        assert_eq!(proc.process(1, &mut out), 1); // fires at tick 3
        // Release.
        proc.process(0, &mut out);
        // Hold again — two ticks with no fire.
        assert_eq!(proc.process(1, &mut out), 0);
        assert_eq!(proc.process(1, &mut out), 0);
        // Third tick fires again.
        assert_eq!(proc.process(1, &mut out), 1);
    }

    // ── Disabled macro does not fire ──────────────────────────────────────────

    #[test]
    fn test_macro_disabled_does_not_fire() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 0 },
            enabled: false,
        })
        .unwrap();

        let mut out = make_output();
        assert_eq!(proc.process(1, &mut out), 0);
    }

    // ── AxisOffset action ─────────────────────────────────────────────────────

    #[test]
    fn test_axis_offset_action() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(2),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::AxisOffset {
                axis_index: 0,
                delta: 0.1,
            },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        let n = proc.process(1 << 2, &mut out);
        assert_eq!(n, 1);
        assert_eq!(
            out[0],
            MacroAction::AxisOffset {
                axis_index: 0,
                delta: 0.1
            }
        );
    }

    // ── AxisSet action ────────────────────────────────────────────────────────

    #[test]
    fn test_axis_set_action() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(3),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::AxisSet {
                axis_index: 1,
                value: -0.5,
            },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        let n = proc.process(1 << 3, &mut out);
        assert_eq!(n, 1);
        assert_eq!(
            out[0],
            MacroAction::AxisSet {
                axis_index: 1,
                value: -0.5
            }
        );
    }

    // ── Suppress action ───────────────────────────────────────────────────────

    #[test]
    fn test_suppress_action() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(4),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::Suppress,
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        let n = proc.process(1 << 4, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], MacroAction::Suppress);
    }

    // ── VirtualButton action ──────────────────────────────────────────────────

    #[test]
    fn test_virtual_button_action() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(5),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 99 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        let n = proc.process(1 << 5, &mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], MacroAction::VirtualButton { index: 99 });
    }

    // ── ButtonChord errors ────────────────────────────────────────────────────

    #[test]
    fn test_chord_too_many_buttons_error() {
        let result = ButtonChord::from_slice(&[0, 1, 2, 3, 4]);
        assert!(matches!(result, Err(ButtonError::TooManyButtons)));
    }

    #[test]
    fn test_empty_chord_error() {
        let result = ButtonChord::from_slice(&[]);
        assert!(matches!(result, Err(ButtonError::EmptyChord)));
    }

    #[test]
    fn test_duplicate_button_error() {
        let result = ButtonChord::from_slice(&[1, 1]);
        assert!(matches!(result, Err(ButtonError::DuplicateButton)));
    }

    // ── Multiple macros fire in the same tick ─────────────────────────────────

    #[test]
    fn test_multiple_macros_same_tick() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 10 },
            enabled: true,
        })
        .unwrap();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(1),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 11 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        // Both buttons pressed simultaneously.
        let n = proc.process(0b11, &mut out);
        assert_eq!(n, 2);
        assert_eq!(out[0], MacroAction::VirtualButton { index: 10 });
        assert_eq!(out[1], MacroAction::VirtualButton { index: 11 });
    }

    // ── set_enabled / clear ───────────────────────────────────────────────────

    #[test]
    fn test_set_enabled_toggles_macro() {
        let mut proc = ButtonProcessor::new();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 0 },
            enabled: true,
        })
        .unwrap();

        let mut out = make_output();
        proc.set_enabled(0, false);
        assert_eq!(proc.process(1, &mut out), 0);
        proc.set_enabled(0, true);
        // Must re-prime prev_state = 0 to get a rising edge.
        proc.clear();
        proc.register_macro(ButtonMacro {
            chord: ButtonChord::single(0),
            trigger: MacroTrigger::OnPress,
            action: MacroAction::VirtualButton { index: 0 },
            enabled: true,
        })
        .unwrap();
        // Reset processor state by releasing all buttons first.
        proc.process(0, &mut out);
        assert_eq!(proc.process(1, &mut out), 1);
    }

    #[test]
    fn test_clear_removes_all_macros() {
        let mut proc = ButtonProcessor::new();
        for i in 0..5u8 {
            proc.register_macro(ButtonMacro {
                chord: ButtonChord::single(i),
                trigger: MacroTrigger::OnPress,
                action: MacroAction::Suppress,
                enabled: true,
            })
            .unwrap();
        }
        assert_eq!(proc.macro_count(), 5);
        proc.clear();
        assert_eq!(proc.macro_count(), 0);
        let mut out = make_output();
        assert_eq!(proc.process(0xFF, &mut out), 0);
    }

    // ── Proptests ─────────────────────────────────────────────────────────────

    proptest! {
        /// `process` must never panic regardless of state sequence.
        #[test]
        fn proptest_process_never_panics(
            states in proptest::collection::vec(any::<u64>(), 1..=32)
        ) {
            let mut proc = ButtonProcessor::new();
            proc.register_macro(ButtonMacro {
                chord: ButtonChord::single(0),
                trigger: MacroTrigger::OnPress,
                action: MacroAction::Suppress,
                enabled: true,
            }).unwrap();
            proc.register_macro(ButtonMacro {
                chord: ButtonChord::from_slice(&[1, 2]).unwrap(),
                trigger: MacroTrigger::OnHold { hold_ticks: 2, interval_ticks: 1 },
                action: MacroAction::VirtualButton { index: 0 },
                enabled: true,
            }).unwrap();

            let mut out = make_output();
            for s in states {
                let _ = proc.process(s, &mut out);
            }
        }

        /// The returned count must never exceed the output buffer length (8).
        #[test]
        fn proptest_output_count_never_exceeds_8(
            states in proptest::collection::vec(any::<u64>(), 1..=16),
            n_macros in 0usize..=20,
        ) {
            let mut proc = ButtonProcessor::new();
            for i in 0..n_macros {
                let btn = (i % 64) as u8;
                proc.register_macro(ButtonMacro {
                    chord: ButtonChord::single(btn),
                    trigger: MacroTrigger::OnPress,
                    action: MacroAction::Suppress,
                    enabled: true,
                }).unwrap();
            }

            let mut out = make_output();
            for s in states {
                let count = proc.process(s, &mut out);
                prop_assert!(count <= 8, "count={count} exceeds 8");
            }
        }
    }
}
