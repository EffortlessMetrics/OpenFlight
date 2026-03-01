// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Button state change detection for Honeycomb devices.
//!
//! [`ButtonDelta`] computes which buttons were newly pressed or released
//! between two consecutive HID reports. This is useful for triggering
//! sim events on button edges rather than polling button state.

/// Describes the changes between two 64-bit button masks.
///
/// Constructed via [`ButtonDelta::compute`] or the convenience methods on
/// [`AlphaInputState`](crate::AlphaInputState) and
/// [`BravoInputState`](crate::BravoInputState).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonDelta {
    /// Bitmask of buttons that transitioned from released to pressed (rising edge).
    pub pressed: u64,
    /// Bitmask of buttons that transitioned from pressed to released (falling edge).
    pub released: u64,
}

impl ButtonDelta {
    /// Compute the delta between a previous and current button mask.
    pub fn compute(prev: u64, current: u64) -> Self {
        Self {
            pressed: current & !prev,
            released: prev & !current,
        }
    }

    /// Returns `true` if no buttons changed state.
    pub fn is_empty(&self) -> bool {
        self.pressed == 0 && self.released == 0
    }

    /// Returns `true` if button `n` (1-indexed) was newly pressed.
    pub fn was_pressed(&self, n: u8) -> bool {
        n >= 1 && (self.pressed >> (n - 1)) & 1 == 1
    }

    /// Returns `true` if button `n` (1-indexed) was newly released.
    pub fn was_released(&self, n: u8) -> bool {
        n >= 1 && (self.released >> (n - 1)) & 1 == 1
    }

    /// Returns the count of newly pressed buttons.
    pub fn pressed_count(&self) -> u32 {
        self.pressed.count_ones()
    }

    /// Returns the count of newly released buttons.
    pub fn released_count(&self) -> u32 {
        self.released.count_ones()
    }

    /// Iterates over 1-indexed button numbers that were newly pressed.
    pub fn pressed_buttons(&self) -> ButtonIter {
        ButtonIter {
            mask: self.pressed,
            pos: 0,
        }
    }

    /// Iterates over 1-indexed button numbers that were newly released.
    pub fn released_buttons(&self) -> ButtonIter {
        ButtonIter {
            mask: self.released,
            pos: 0,
        }
    }
}

/// Iterator over set bits in a 64-bit mask, yielding 1-indexed button numbers.
pub struct ButtonIter {
    mask: u64,
    pos: u8,
}

impl Iterator for ButtonIter {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        while self.pos < 64 {
            let bit = self.pos;
            self.pos += 1;
            if (self.mask >> bit) & 1 == 1 {
                return Some(bit + 1); // 1-indexed
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_change_produces_empty_delta() {
        let delta = ButtonDelta::compute(0xFF, 0xFF);
        assert!(delta.is_empty());
        assert_eq!(delta.pressed_count(), 0);
        assert_eq!(delta.released_count(), 0);
    }

    #[test]
    fn single_button_press() {
        let delta = ButtonDelta::compute(0, 1 << 0);
        assert!(delta.was_pressed(1));
        assert!(!delta.was_released(1));
        assert_eq!(delta.pressed_count(), 1);
    }

    #[test]
    fn single_button_release() {
        let delta = ButtonDelta::compute(1 << 0, 0);
        assert!(!delta.was_pressed(1));
        assert!(delta.was_released(1));
        assert_eq!(delta.released_count(), 1);
    }

    #[test]
    fn multiple_simultaneous_changes() {
        let prev: u64 = 0b1100;
        let curr: u64 = 0b0011;
        let delta = ButtonDelta::compute(prev, curr);
        assert!(delta.was_pressed(1)); // bit 0: 0→1
        assert!(delta.was_pressed(2)); // bit 1: 0→1
        assert!(delta.was_released(3)); // bit 2: 1→0
        assert!(delta.was_released(4)); // bit 3: 1→0
        assert_eq!(delta.pressed_count(), 2);
        assert_eq!(delta.released_count(), 2);
    }

    #[test]
    fn pressed_buttons_iterator() {
        let delta = ButtonDelta::compute(0, 0b1010);
        let pressed: Vec<u8> = delta.pressed_buttons().collect();
        assert_eq!(pressed, vec![2, 4]); // bits 1 and 3 → buttons 2 and 4
    }

    #[test]
    fn released_buttons_iterator() {
        let delta = ButtonDelta::compute(0b0101, 0);
        let released: Vec<u8> = delta.released_buttons().collect();
        assert_eq!(released, vec![1, 3]); // bits 0 and 2 → buttons 1 and 3
    }

    #[test]
    fn high_bit_button() {
        let prev: u64 = 0;
        let curr: u64 = 1 << 63;
        let delta = ButtonDelta::compute(prev, curr);
        assert!(delta.was_pressed(64));
        assert_eq!(delta.pressed_count(), 1);
    }

    #[test]
    fn zero_button_number_returns_false() {
        let delta = ButtonDelta::compute(0, u64::MAX);
        assert!(!delta.was_pressed(0));
        assert!(!delta.was_released(0));
    }

    #[test]
    fn all_buttons_pressed_at_once() {
        let delta = ButtonDelta::compute(0, u64::MAX);
        assert_eq!(delta.pressed_count(), 64);
        assert_eq!(delta.released_count(), 0);
        assert!(delta.was_pressed(1));
        assert!(delta.was_pressed(64));
    }

    #[test]
    fn all_buttons_released_at_once() {
        let delta = ButtonDelta::compute(u64::MAX, 0);
        assert_eq!(delta.pressed_count(), 0);
        assert_eq!(delta.released_count(), 64);
    }

    #[test]
    fn held_buttons_not_in_delta() {
        let mask: u64 = 0b1111;
        let delta = ButtonDelta::compute(mask, mask);
        assert!(!delta.was_pressed(1));
        assert!(!delta.was_released(1));
        assert!(delta.is_empty());
    }

    #[test]
    fn bravo_gear_up_press_detection() {
        let prev: u64 = 0;
        let curr: u64 = 1 << 30; // gear up = bit 30 = button 31
        let delta = ButtonDelta::compute(prev, curr);
        assert!(delta.was_pressed(31));
    }

    #[test]
    fn bravo_ap_master_release_detection() {
        let prev: u64 = 1 << 7; // AP master = bit 7 = button 8
        let curr: u64 = 0;
        let delta = ButtonDelta::compute(prev, curr);
        assert!(delta.was_released(8));
    }

    #[test]
    fn iterator_is_sorted_ascending() {
        let delta = ButtonDelta::compute(0, 0xFF_FFFF_FFFF);
        let pressed: Vec<u8> = delta.pressed_buttons().collect();
        for window in pressed.windows(2) {
            assert!(window[0] < window[1], "iterator not sorted");
        }
    }
}
