// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Button state change detection for VKB devices.
//!
//! Compares consecutive poll snapshots and emits press/release events.

/// A single button state change event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonEvent {
    /// 1-based button number.
    pub button: u16,
    /// Whether the button transitioned to pressed (`true`) or released (`false`).
    pub pressed: bool,
}

/// Detect button changes between two boolean button arrays.
///
/// Returns a `Vec` of [`ButtonEvent`] for every button whose state differs
/// between `previous` and `current`. Button numbering is 1-based.
pub fn detect_button_changes(previous: &[bool], current: &[bool]) -> Vec<ButtonEvent> {
    let len = previous.len().min(current.len());
    let mut events = Vec::new();
    for i in 0..len {
        if previous[i] != current[i] {
            events.push(ButtonEvent {
                button: (i + 1) as u16,
                pressed: current[i],
            });
        }
    }
    events
}

/// Detect button changes between two packed u32 button words.
///
/// Compares bit-by-bit and returns events for each changed bit.
/// Button numbering is 1-based starting from `base_button`.
pub fn detect_button_word_changes(
    previous: u32,
    current: u32,
    base_button: u16,
) -> Vec<ButtonEvent> {
    let diff = previous ^ current;
    if diff == 0 {
        return Vec::new();
    }
    let mut events = Vec::new();
    for bit in 0..32u16 {
        if (diff >> bit) & 1 != 0 {
            events.push(ButtonEvent {
                button: base_button + bit,
                pressed: (current >> bit) & 1 != 0,
            });
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes_returns_empty() {
        let state = [false, true, false, true];
        assert!(detect_button_changes(&state, &state).is_empty());
    }

    #[test]
    fn single_press_detected() {
        let prev = [false, false, false, false];
        let curr = [false, true, false, false];
        let events = detect_button_changes(&prev, &curr);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ButtonEvent {
                button: 2,
                pressed: true
            }
        );
    }

    #[test]
    fn single_release_detected() {
        let prev = [true, false, false, false];
        let curr = [false, false, false, false];
        let events = detect_button_changes(&prev, &curr);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ButtonEvent {
                button: 1,
                pressed: false
            }
        );
    }

    #[test]
    fn multiple_changes_detected() {
        let prev = [true, false, true, false];
        let curr = [false, true, true, true];
        let events = detect_button_changes(&prev, &curr);
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            ButtonEvent {
                button: 1,
                pressed: false
            }
        );
        assert_eq!(
            events[1],
            ButtonEvent {
                button: 2,
                pressed: true
            }
        );
        assert_eq!(
            events[2],
            ButtonEvent {
                button: 4,
                pressed: true
            }
        );
    }

    #[test]
    fn word_no_changes() {
        assert!(detect_button_word_changes(0xDEAD, 0xDEAD, 1).is_empty());
    }

    #[test]
    fn word_single_bit_press() {
        let events = detect_button_word_changes(0x0000_0000, 0x0000_0004, 1);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ButtonEvent {
                button: 3,
                pressed: true
            }
        );
    }

    #[test]
    fn word_with_base_offset() {
        let events = detect_button_word_changes(0x0000_0001, 0x0000_0000, 33);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ButtonEvent {
                button: 33,
                pressed: false
            }
        );
    }

    #[test]
    fn word_multiple_changes() {
        // bits 0 and 1 flip
        let events = detect_button_word_changes(0x0000_0001, 0x0000_0002, 1);
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0],
            ButtonEvent {
                button: 1,
                pressed: false
            }
        );
        assert_eq!(
            events[1],
            ButtonEvent {
                button: 2,
                pressed: true
            }
        );
    }
}
