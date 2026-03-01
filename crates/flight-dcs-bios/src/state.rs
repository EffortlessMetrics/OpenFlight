// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS-BIOS cockpit state tracker.
//!
//! Maintains a 65536-byte memory map representing the current cockpit state.
//! Applies updates from parsed frames and provides typed read access using
//! module definitions.

use std::collections::HashSet;

use crate::controls::{DcsBiosModule, OutputType};
use crate::protocol::{ADDRESS_SPACE_SIZE, DcsBiosUpdate};

/// Cockpit state memory map with change detection.
pub struct DcsBiosState {
    /// The 65536-byte flat memory map.
    memory: Vec<u8>,
    /// Set of addresses that changed in the most recent frame.
    changed_addresses: HashSet<u16>,
    /// Total number of updates applied.
    update_count: u64,
}

impl Default for DcsBiosState {
    fn default() -> Self {
        Self::new()
    }
}

impl DcsBiosState {
    /// Create a new state tracker with zeroed memory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            memory: vec![0u8; ADDRESS_SPACE_SIZE],
            changed_addresses: HashSet::new(),
            update_count: 0,
        }
    }

    /// Apply a single update to the memory map.
    ///
    /// Tracks which addresses were modified for change detection.
    pub fn apply_update(&mut self, update: &DcsBiosUpdate) {
        let start = update.address as usize;
        let end = start + update.data.len();

        if end > ADDRESS_SPACE_SIZE {
            tracing::warn!(
                address = update.address,
                length = update.data.len(),
                "DCS-BIOS update exceeds address space, ignoring"
            );
            return;
        }

        for (i, &byte) in update.data.iter().enumerate() {
            let addr = start + i;
            if self.memory[addr] != byte {
                self.memory[addr] = byte;
                // Track the 16-bit aligned address
                self.changed_addresses.insert((addr & !1) as u16);
            }
        }
        self.update_count += 1;
    }

    /// Apply all updates from a parsed frame.
    pub fn apply_updates(&mut self, updates: &[DcsBiosUpdate]) {
        for update in updates {
            self.apply_update(update);
        }
    }

    /// Clear the change tracking set (call after processing changes).
    pub fn clear_changes(&mut self) {
        self.changed_addresses.clear();
    }

    /// Check if a specific address changed since the last `clear_changes`.
    #[must_use]
    pub fn has_changed(&self, address: u16) -> bool {
        self.changed_addresses.contains(&address)
    }

    /// Get all addresses that changed since the last `clear_changes`.
    #[must_use]
    pub fn changed_addresses(&self) -> &HashSet<u16> {
        &self.changed_addresses
    }

    /// Read a 16-bit word at the given address.
    ///
    /// Returns `None` if the address is out of bounds or misaligned.
    #[must_use]
    pub fn read_u16(&self, address: u16) -> Option<u16> {
        let addr = address as usize;
        if addr + 2 > ADDRESS_SPACE_SIZE || !addr.is_multiple_of(2) {
            return None;
        }
        Some(u16::from_le_bytes([
            self.memory[addr],
            self.memory[addr + 1],
        ]))
    }

    /// Read an integer control value by name using a module definition.
    ///
    /// Looks up the control's address/mask/shift and decodes the value.
    #[must_use]
    pub fn read_integer(&self, module: &DcsBiosModule, identifier: &str) -> Option<u16> {
        let addr_desc = module.integer_output(identifier)?;
        let word = self.read_u16(addr_desc.address)?;
        Some(addr_desc.decode(word))
    }

    /// Read a string control value by name using a module definition.
    ///
    /// Returns the string trimmed of trailing null bytes.
    #[must_use]
    pub fn read_string(&self, module: &DcsBiosModule, identifier: &str) -> Option<String> {
        let (address, max_length) = module.string_output(identifier)?;
        let start = address as usize;
        let end = start + max_length as usize;
        if end > ADDRESS_SPACE_SIZE {
            return None;
        }
        let bytes = &self.memory[start..end];
        let s = String::from_utf8_lossy(bytes);
        Some(s.trim_end_matches('\0').to_owned())
    }

    /// Read raw bytes from the memory map.
    #[must_use]
    pub fn read_bytes(&self, address: u16, length: u16) -> Option<&[u8]> {
        let start = address as usize;
        let end = start + length as usize;
        if end > ADDRESS_SPACE_SIZE {
            return None;
        }
        Some(&self.memory[start..end])
    }

    /// Get the total number of updates applied.
    #[must_use]
    pub fn update_count(&self) -> u64 {
        self.update_count
    }

    /// Reset all memory to zero and clear change tracking.
    pub fn reset(&mut self) {
        self.memory.fill(0);
        self.changed_addresses.clear();
        self.update_count = 0;
    }

    /// Get all controls in a module that changed since last `clear_changes`.
    #[must_use]
    pub fn changed_controls<'a>(&self, module: &'a DcsBiosModule) -> Vec<&'a str> {
        let mut result = Vec::new();
        for (id, control) in &module.controls {
            for output in &control.outputs {
                let addr = match output {
                    OutputType::Integer(a) => a.address,
                    OutputType::String { address, .. } => *address,
                };
                if self.changed_addresses.contains(&addr) {
                    result.push(id.as_str());
                    break;
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::*;

    fn test_module() -> DcsBiosModule {
        let mut module = DcsBiosModule::new("test", 0x7400, &["TestAircraft"]);
        module.add_control(toggle_switch(
            "MASTER_ARM",
            "Armament",
            "Master Arm Switch",
            0x7408,
            0x0100,
            8,
        ));
        module.add_control(string_display(
            "FUEL_DISPLAY",
            "IFEI",
            "Fuel Display",
            0x7480,
            6,
        ));
        module.add_control(selector(
            "FLAP_SW",
            "Flight Controls",
            "Flap Switch",
            0x740A,
            0x0600,
            9,
            2,
        ));
        module
    }

    #[test]
    fn new_state_is_zeroed() {
        let state = DcsBiosState::new();
        assert_eq!(state.read_u16(0x0000), Some(0));
        assert_eq!(state.read_u16(0x7400), Some(0));
        assert_eq!(state.update_count(), 0);
    }

    #[test]
    fn apply_update_writes_memory() {
        let mut state = DcsBiosState::new();
        let update = DcsBiosUpdate {
            address: 0x1000,
            data: vec![0xAB, 0xCD],
        };
        state.apply_update(&update);
        assert_eq!(state.read_u16(0x1000), Some(0xCDAB));
        assert_eq!(state.update_count(), 1);
    }

    #[test]
    fn apply_update_tracks_changes() {
        let mut state = DcsBiosState::new();
        let update = DcsBiosUpdate {
            address: 0x1000,
            data: vec![0x01, 0x00],
        };
        state.apply_update(&update);
        assert!(state.has_changed(0x1000));
        assert!(!state.has_changed(0x1002));
    }

    #[test]
    fn clear_changes_resets_tracking() {
        let mut state = DcsBiosState::new();
        state.apply_update(&DcsBiosUpdate {
            address: 0x1000,
            data: vec![0x01, 0x00],
        });
        assert!(state.has_changed(0x1000));

        state.clear_changes();
        assert!(!state.has_changed(0x1000));
    }

    #[test]
    fn same_value_write_not_tracked_as_change() {
        let mut state = DcsBiosState::new();
        // Memory is already zero, writing zero should not trigger change
        state.apply_update(&DcsBiosUpdate {
            address: 0x1000,
            data: vec![0x00, 0x00],
        });
        assert!(!state.has_changed(0x1000));
    }

    #[test]
    fn read_integer_via_module() {
        let module = test_module();
        let mut state = DcsBiosState::new();

        // Write 0x0100 to address 0x7408 (MASTER_ARM: mask=0x0100, shift=8 → value=1)
        state.apply_update(&DcsBiosUpdate {
            address: 0x7408,
            data: vec![0x00, 0x01],
        });

        assert_eq!(state.read_integer(&module, "MASTER_ARM"), Some(1));
    }

    #[test]
    fn read_integer_zero() {
        let module = test_module();
        let state = DcsBiosState::new();
        assert_eq!(state.read_integer(&module, "MASTER_ARM"), Some(0));
    }

    #[test]
    fn read_string_via_module() {
        let module = test_module();
        let mut state = DcsBiosState::new();

        // Write "12345\0" to address 0x7480
        state.apply_update(&DcsBiosUpdate {
            address: 0x7480,
            data: vec![0x31, 0x32, 0x33, 0x34, 0x35, 0x00],
        });

        assert_eq!(
            state.read_string(&module, "FUEL_DISPLAY"),
            Some("12345".to_owned())
        );
    }

    #[test]
    fn read_string_partial() {
        let module = test_module();
        let mut state = DcsBiosState::new();

        // Write "AB" to address 0x7480, rest stays zero
        state.apply_update(&DcsBiosUpdate {
            address: 0x7480,
            data: vec![0x41, 0x42],
        });

        assert_eq!(
            state.read_string(&module, "FUEL_DISPLAY"),
            Some("AB".to_owned())
        );
    }

    #[test]
    fn read_nonexistent_control_returns_none() {
        let module = test_module();
        let state = DcsBiosState::new();
        assert_eq!(state.read_integer(&module, "NONEXISTENT"), None);
        assert_eq!(state.read_string(&module, "NONEXISTENT"), None);
    }

    #[test]
    fn read_u16_misaligned_returns_none() {
        let state = DcsBiosState::new();
        assert_eq!(state.read_u16(0x0001), None);
    }

    #[test]
    fn reset_clears_everything() {
        let mut state = DcsBiosState::new();
        state.apply_update(&DcsBiosUpdate {
            address: 0x1000,
            data: vec![0xFF, 0xFF],
        });
        assert_eq!(state.read_u16(0x1000), Some(0xFFFF));

        state.reset();
        assert_eq!(state.read_u16(0x1000), Some(0x0000));
        assert_eq!(state.update_count(), 0);
        assert!(state.changed_addresses().is_empty());
    }

    #[test]
    fn changed_controls_detects_modifications() {
        let module = test_module();
        let mut state = DcsBiosState::new();

        state.apply_update(&DcsBiosUpdate {
            address: 0x7408,
            data: vec![0x00, 0x01],
        });

        let changed = state.changed_controls(&module);
        assert!(changed.contains(&"MASTER_ARM"));
    }

    #[test]
    fn apply_updates_batch() {
        let mut state = DcsBiosState::new();
        let updates = vec![
            DcsBiosUpdate {
                address: 0x0000,
                data: vec![0x01, 0x00],
            },
            DcsBiosUpdate {
                address: 0x0002,
                data: vec![0x02, 0x00],
            },
        ];
        state.apply_updates(&updates);
        assert_eq!(state.read_u16(0x0000), Some(0x0001));
        assert_eq!(state.read_u16(0x0002), Some(0x0002));
        assert_eq!(state.update_count(), 2);
    }

    #[test]
    fn read_bytes_returns_slice() {
        let mut state = DcsBiosState::new();
        state.apply_update(&DcsBiosUpdate {
            address: 0x2000,
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        });
        let bytes = state.read_bytes(0x2000, 4).unwrap();
        assert_eq!(bytes, &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn read_bytes_out_of_bounds() {
        let state = DcsBiosState::new();
        assert!(state.read_bytes(0xFFFF, 2).is_none());
    }

    #[test]
    fn multi_bit_selector_read() {
        let module = test_module();
        let mut state = DcsBiosState::new();

        // FLAP_SW: address=0x740A, mask=0x0600, shift=9
        // Value 2 → bits 10:9 = 10 → word = 0x0400
        state.apply_update(&DcsBiosUpdate {
            address: 0x740A,
            data: vec![0x00, 0x04],
        });

        assert_eq!(state.read_integer(&module, "FLAP_SW"), Some(2));
    }

    #[test]
    fn default_impl_matches_new() {
        let a = DcsBiosState::new();
        let b = DcsBiosState::default();
        assert_eq!(a.update_count(), b.update_count());
        assert_eq!(a.read_u16(0x0000), b.read_u16(0x0000));
    }
}
