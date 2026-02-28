// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Full HID report descriptor parser.
//!
//! Parses raw USB HID report descriptors (per USB HID spec 1.11 §6.2.2) to
//! extract axis, button, and hat-switch information including logical/physical
//! ranges, report sizes, and collection hierarchy.
//!
//! This module complements the lightweight usage extractor in
//! [`flight_hid_support::hid_descriptor`] by providing a richer parse tree
//! suitable for calibration and device-capability queries.

use std::fmt;
use thiserror::Error;

// ── HID item tag constants (per HID spec 1.11 §6.2.2) ────────────────────

// Main items
const MAIN_INPUT: u8 = 0x08;
const MAIN_OUTPUT: u8 = 0x09;
const MAIN_FEATURE: u8 = 0x0B;
const MAIN_COLLECTION: u8 = 0x0A;
const MAIN_END_COLLECTION: u8 = 0x0C;

// Global items
const GLOBAL_USAGE_PAGE: u8 = 0x00;
const GLOBAL_LOGICAL_MIN: u8 = 0x01;
const GLOBAL_LOGICAL_MAX: u8 = 0x02;
const GLOBAL_PHYSICAL_MIN: u8 = 0x03;
const GLOBAL_PHYSICAL_MAX: u8 = 0x04;
const GLOBAL_REPORT_SIZE: u8 = 0x07;
const GLOBAL_REPORT_COUNT: u8 = 0x09;

// Local items
const LOCAL_USAGE: u8 = 0x00;
const LOCAL_USAGE_MIN: u8 = 0x01;
const LOCAL_USAGE_MAX: u8 = 0x02;

// Long-item sentinel
const HID_ITEM_LONG: u8 = 0xFE;

// Well-known usage pages & usages
const USAGE_PAGE_GENERIC_DESKTOP: u16 = 0x01;
const USAGE_PAGE_BUTTON: u16 = 0x09;
const USAGE_HAT_SWITCH: u16 = 0x39;

// ── Error type ────────────────────────────────────────────────────────────

/// Errors produced during HID descriptor parsing.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DescriptorError {
    /// The descriptor byte stream is empty.
    #[error("empty descriptor")]
    Empty,

    /// An item header references data beyond the end of the descriptor.
    #[error("truncated item at byte offset {offset}")]
    Truncated { offset: usize },

    /// A collection End was encountered without a matching Begin.
    #[error("unmatched end-collection at byte offset {offset}")]
    UnmatchedEnd { offset: usize },

    /// The descriptor ended with unclosed collections.
    #[error("unclosed collection(s): {count} remaining")]
    UnclosedCollection { count: usize },
}

// ── Public types ──────────────────────────────────────────────────────────

/// The type of a HID collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionType {
    Application,
    Physical,
    Logical,
    Report,
    /// Any value not covered above.
    Other(u32),
}

impl CollectionType {
    fn from_value(v: u32) -> Self {
        match v {
            0x01 => Self::Application,
            0x00 => Self::Physical,
            0x02 => Self::Logical,
            0x03 => Self::Report,
            other => Self::Other(other),
        }
    }
}

/// Classification of a HID main item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidItemType {
    Input,
    Output,
    Feature,
}

/// A single parsed HID data field (axis, button group, hat, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidItem {
    pub item_type: HidItemType,
    pub usage_page: u16,
    pub usage: u16,
    pub logical_min: i32,
    pub logical_max: i32,
    pub physical_min: i32,
    pub physical_max: i32,
    pub report_size: u32,
    pub report_count: u32,
}

/// A HID collection with its contained items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidCollection {
    pub usage_page: u16,
    pub usage: u16,
    pub collection_type: CollectionType,
    pub items: Vec<HidItem>,
}

/// Top-level result of parsing a complete HID report descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HidDescriptor {
    pub collections: Vec<HidCollection>,
    pub total_axes: u32,
    pub total_buttons: u32,
    pub total_hats: u32,
    pub report_size_bits: u32,
}

impl HidDescriptor {
    /// Number of axes reported by the device.
    pub fn axis_count(&self) -> u32 {
        self.total_axes
    }

    /// Number of buttons reported by the device.
    pub fn button_count(&self) -> u32 {
        self.total_buttons
    }

    /// Number of hat switches reported by the device.
    pub fn hat_count(&self) -> u32 {
        self.total_hats
    }

    /// Logical (min, max) range for each axis, in descriptor order.
    pub fn axis_ranges(&self) -> Vec<(i32, i32)> {
        let mut ranges = Vec::new();
        for col in &self.collections {
            for item in &col.items {
                if item.item_type == HidItemType::Input && is_axis_item(item) {
                    for _ in 0..item.report_count {
                        ranges.push((item.logical_min, item.logical_max));
                    }
                }
            }
        }
        ranges
    }
}

impl fmt::Display for HidDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "HidDescriptor {{ axes: {}, buttons: {}, hats: {}, bits: {} }}",
            self.total_axes, self.total_buttons, self.total_hats, self.report_size_bits
        )
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────

/// Global state carried across items while parsing.
#[derive(Clone, Default)]
struct GlobalState {
    usage_page: u16,
    logical_min: i32,
    logical_max: i32,
    physical_min: i32,
    physical_max: i32,
    report_size: u32,
    report_count: u32,
}

/// Local state reset after each main item.
#[derive(Default)]
struct LocalState {
    usages: Vec<u16>,
    usage_min: Option<u16>,
    usage_max: Option<u16>,
}

impl LocalState {
    fn clear(&mut self) {
        self.usages.clear();
        self.usage_min = None;
        self.usage_max = None;
    }

    /// Expand into a usage list. If min/max range is set, expand the range;
    /// otherwise return the individually declared usages.
    fn expanded_usages(&self) -> Vec<u16> {
        if let (Some(min), Some(max)) = (self.usage_min, self.usage_max) {
            let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
            (lo..=hi).collect()
        } else {
            self.usages.clone()
        }
    }
}

fn item_size(size_code: u8) -> usize {
    match size_code {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    }
}

/// Read a little-endian unsigned value from `data` (0–4 bytes).
fn read_unsigned(data: &[u8]) -> u32 {
    let mut v = 0u32;
    for (i, &b) in data.iter().enumerate() {
        v |= (b as u32) << (i * 8);
    }
    v
}

/// Read a little-endian *signed* value from `data` (0–4 bytes).
fn read_signed(data: &[u8]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    let unsigned = read_unsigned(data);
    let bits = data.len() * 8;
    // Sign-extend: if the MSB of the data is set, fill upper bits with 1s.
    if bits < 32 && (unsigned >> (bits - 1)) & 1 == 1 {
        (unsigned | (!0u32 << bits)) as i32
    } else {
        unsigned as i32
    }
}

/// Returns `true` when an Input item describes an axis (not a button or hat).
fn is_axis_item(item: &HidItem) -> bool {
    if item.usage_page == USAGE_PAGE_BUTTON {
        return false;
    }
    if item.usage_page == USAGE_PAGE_GENERIC_DESKTOP && item.usage == USAGE_HAT_SWITCH {
        return false;
    }
    // Axes are multi-bit fields (usually > 1 bit).
    item.report_size > 1
}

fn is_button_item(item: &HidItem) -> bool {
    item.usage_page == USAGE_PAGE_BUTTON
}

fn is_hat_item(item: &HidItem) -> bool {
    item.usage_page == USAGE_PAGE_GENERIC_DESKTOP && item.usage == USAGE_HAT_SWITCH
}

// ── Public API ────────────────────────────────────────────────────────────

/// Parse a raw HID report descriptor into a structured [`HidDescriptor`].
pub fn parse_descriptor(bytes: &[u8]) -> Result<HidDescriptor, DescriptorError> {
    if bytes.is_empty() {
        return Err(DescriptorError::Empty);
    }

    let mut global = GlobalState::default();
    let mut local = LocalState::default();
    let mut global_stack: Vec<GlobalState> = Vec::new();

    // Collection stack: we accumulate items into the top collection.
    let mut col_stack: Vec<HidCollection> = Vec::new();
    let mut finished_collections: Vec<HidCollection> = Vec::new();

    let mut total_axes: u32 = 0;
    let mut total_buttons: u32 = 0;
    let mut total_hats: u32 = 0;
    let mut report_bits: u32 = 0;

    let mut idx = 0usize;
    while idx < bytes.len() {
        let prefix = bytes[idx];
        let item_offset = idx;
        idx += 1;

        // Long items (rare, skip payload)
        if prefix == HID_ITEM_LONG {
            if idx >= bytes.len() {
                return Err(DescriptorError::Truncated {
                    offset: item_offset,
                });
            }
            let data_len = bytes[idx] as usize;
            idx += 2; // length byte + long-item tag
            if idx.saturating_add(data_len) > bytes.len() {
                return Err(DescriptorError::Truncated {
                    offset: item_offset,
                });
            }
            idx += data_len;
            continue;
        }

        let size_code = prefix & 0x03;
        let item_type = (prefix >> 2) & 0x03;
        let tag = (prefix >> 4) & 0x0F;
        let size = item_size(size_code);

        if idx + size > bytes.len() {
            return Err(DescriptorError::Truncated {
                offset: item_offset,
            });
        }

        let data = &bytes[idx..idx + size];
        idx += size;

        match item_type {
            // ── Main ──────────────────────────────────────────────────
            0x00 => match tag {
                MAIN_INPUT | MAIN_OUTPUT | MAIN_FEATURE => {
                    let it = match tag {
                        MAIN_INPUT => HidItemType::Input,
                        MAIN_OUTPUT => HidItemType::Output,
                        _ => HidItemType::Feature,
                    };

                    let item_flags = read_unsigned(data);
                    let is_constant = item_flags & 0x01 != 0;

                    let usages = local.expanded_usages();
                    let primary_usage = usages.first().copied().unwrap_or(0);

                    let item = HidItem {
                        item_type: it,
                        usage_page: global.usage_page,
                        usage: primary_usage,
                        logical_min: global.logical_min,
                        logical_max: global.logical_max,
                        physical_min: global.physical_min,
                        physical_max: global.physical_max,
                        report_size: global.report_size,
                        report_count: global.report_count,
                    };

                    // Classify and count (skip constant/padding items)
                    if it == HidItemType::Input && !is_constant {
                        if is_button_item(&item) {
                            total_buttons += global.report_count;
                        } else if is_hat_item(&item) {
                            total_hats += global.report_count;
                        } else if is_axis_item(&item) {
                            total_axes += global.report_count;
                        }
                    }
                    if it == HidItemType::Input {
                        report_bits += global.report_size * global.report_count;
                    }

                    if let Some(col) = col_stack.last_mut() {
                        col.items.push(item);
                    }

                    local.clear();
                }
                MAIN_COLLECTION => {
                    let ctype = CollectionType::from_value(read_unsigned(data));
                    let primary_usage = local.expanded_usages();
                    let usage = primary_usage.first().copied().unwrap_or(0);
                    col_stack.push(HidCollection {
                        usage_page: global.usage_page,
                        usage,
                        collection_type: ctype,
                        items: Vec::new(),
                    });
                    local.clear();
                }
                MAIN_END_COLLECTION => {
                    let col = col_stack.pop().ok_or(DescriptorError::UnmatchedEnd {
                        offset: item_offset,
                    })?;
                    if col_stack.is_empty() {
                        finished_collections.push(col);
                    } else if let Some(parent) = col_stack.last_mut() {
                        // Flatten: merge child items into parent for simplicity.
                        parent.items.extend(col.items);
                    }
                    local.clear();
                }
                _ => {
                    local.clear();
                }
            },
            // ── Global ────────────────────────────────────────────────
            0x01 => match tag {
                GLOBAL_USAGE_PAGE => global.usage_page = read_unsigned(data) as u16,
                GLOBAL_LOGICAL_MIN => global.logical_min = read_signed(data),
                GLOBAL_LOGICAL_MAX => global.logical_max = read_signed(data),
                GLOBAL_PHYSICAL_MIN => global.physical_min = read_signed(data),
                GLOBAL_PHYSICAL_MAX => global.physical_max = read_signed(data),
                GLOBAL_REPORT_SIZE => global.report_size = read_unsigned(data),
                GLOBAL_REPORT_COUNT => global.report_count = read_unsigned(data),
                // Push (0x0A) / Pop (0x0B)
                0x0A => global_stack.push(global.clone()),
                0x0B => {
                    if let Some(g) = global_stack.pop() {
                        global = g;
                    }
                }
                _ => {}
            },
            // ── Local ─────────────────────────────────────────────────
            0x02 => match tag {
                LOCAL_USAGE => local.usages.push(read_unsigned(data) as u16),
                LOCAL_USAGE_MIN => local.usage_min = Some(read_unsigned(data) as u16),
                LOCAL_USAGE_MAX => local.usage_max = Some(read_unsigned(data) as u16),
                _ => {}
            },
            _ => {}
        }
    }

    if !col_stack.is_empty() {
        return Err(DescriptorError::UnclosedCollection {
            count: col_stack.len(),
        });
    }

    Ok(HidDescriptor {
        collections: finished_collections,
        total_axes,
        total_buttons,
        total_hats,
        report_size_bits: report_bits,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a tiny HID descriptor for a joystick with the given
    /// number of axes and buttons.
    fn simple_joystick_descriptor(axes: u8, buttons: u8) -> Vec<u8> {
        let mut d = vec![
            0x05, 0x01, // Usage Page (Generic Desktop)
            0x09, 0x04, // Usage (Joystick)
            0xA1, 0x01, // Collection (Application)
        ];

        // --- Axes ---
        if axes > 0 {
            // Usage Page (Generic Desktop)
            d.push(0x05);
            d.push(0x01);
            // Usage Minimum (X = 0x30)
            d.push(0x19);
            d.push(0x30);
            // Usage Maximum
            d.push(0x29);
            d.push(0x30 + axes - 1);
            // Logical Minimum (0)
            d.push(0x15);
            d.push(0x00);
            // Logical Maximum (1023) = 0x03FF
            d.push(0x26);
            d.push(0xFF);
            d.push(0x03);
            // Report Size (16)
            d.push(0x75);
            d.push(0x10);
            // Report Count (axes)
            d.push(0x95);
            d.push(axes);
            // Input (Data, Variable, Absolute)
            d.push(0x81);
            d.push(0x02);
        }

        // --- Buttons ---
        if buttons > 0 {
            // Usage Page (Button)
            d.push(0x05);
            d.push(0x09);
            // Usage Minimum (1)
            d.push(0x19);
            d.push(0x01);
            // Usage Maximum (buttons)
            d.push(0x29);
            d.push(buttons);
            // Logical Minimum (0)
            d.push(0x15);
            d.push(0x00);
            // Logical Maximum (1)
            d.push(0x25);
            d.push(0x01);
            // Report Size (1)
            d.push(0x75);
            d.push(0x01);
            // Report Count (buttons)
            d.push(0x95);
            d.push(buttons);
            // Input (Data, Variable, Absolute)
            d.push(0x81);
            d.push(0x02);

            // Padding to byte boundary
            let pad = (8 - (buttons % 8)) % 8;
            if pad > 0 {
                // Report Size (1)
                d.push(0x75);
                d.push(0x01);
                // Report Count (pad)
                d.push(0x95);
                d.push(pad);
                // Input (Constant)
                d.push(0x81);
                d.push(0x01);
            }
        }

        // End Collection
        d.push(0xC0);

        d
    }

    /// Helper: build a HOTAS-style descriptor with axes, buttons, and hats.
    fn hotas_descriptor(axes: u8, buttons: u8, hats: u8) -> Vec<u8> {
        let mut d = Vec::new();

        // Usage Page (Generic Desktop) / Usage (Joystick) / Collection (Application)
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);

        // --- Axes ---
        if axes > 0 {
            d.extend_from_slice(&[0x05, 0x01]); // Usage Page (Generic Desktop)
            d.push(0x19);
            d.push(0x30); // Usage Min (X)
            d.push(0x29);
            d.push(0x30 + axes - 1); // Usage Max
            d.extend_from_slice(&[0x15, 0x00]); // Logical Min (0)
            d.extend_from_slice(&[0x26, 0xFF, 0x03]); // Logical Max (1023)
            d.push(0x75);
            d.push(0x10); // Report Size (16)
            d.push(0x95);
            d.push(axes); // Report Count
            d.extend_from_slice(&[0x81, 0x02]); // Input
        }

        // --- Hat switch(es) ---
        if hats > 0 {
            d.extend_from_slice(&[0x05, 0x01]); // Usage Page (Generic Desktop)
            d.push(0x09);
            d.push(USAGE_HAT_SWITCH as u8); // Usage (Hat Switch)
            d.extend_from_slice(&[0x15, 0x01]); // Logical Min (1)
            d.extend_from_slice(&[0x25, 0x08]); // Logical Max (8)
            d.push(0x75);
            d.push(0x04); // Report Size (4)
            d.push(0x95);
            d.push(hats); // Report Count
            d.extend_from_slice(&[0x81, 0x42]); // Input (Data, Variable, Absolute, Null state)
        }

        // --- Buttons ---
        if buttons > 0 {
            d.extend_from_slice(&[0x05, 0x09]); // Usage Page (Button)
            d.push(0x19);
            d.push(0x01); // Usage Min
            d.push(0x29);
            d.push(buttons); // Usage Max
            d.extend_from_slice(&[0x15, 0x00]); // Logical Min (0)
            d.extend_from_slice(&[0x25, 0x01]); // Logical Max (1)
            d.push(0x75);
            d.push(0x01); // Report Size (1)
            d.push(0x95);
            d.push(buttons); // Report Count
            d.extend_from_slice(&[0x81, 0x02]); // Input

            let pad = (8 - (buttons % 8)) % 8;
            if pad > 0 {
                d.push(0x75);
                d.push(0x01);
                d.push(0x95);
                d.push(pad);
                d.extend_from_slice(&[0x81, 0x01]);
            }
        }

        // End Collection
        d.push(0xC0);
        d
    }

    // ── Basic tests ───────────────────────────────────────────────────

    #[test]
    fn empty_descriptor_returns_error() {
        assert_eq!(parse_descriptor(&[]), Err(DescriptorError::Empty));
    }

    #[test]
    fn truncated_item_returns_error() {
        // Usage Page (Generic Desktop) with missing data byte
        let d = [0x05]; // expects 1 data byte
        assert!(matches!(
            parse_descriptor(&d),
            Err(DescriptorError::Truncated { .. })
        ));
    }

    #[test]
    fn unmatched_end_collection() {
        let d = [0xC0]; // End Collection with no open collection
        assert!(matches!(
            parse_descriptor(&d),
            Err(DescriptorError::UnmatchedEnd { .. })
        ));
    }

    #[test]
    fn unclosed_collection() {
        // Open a collection without closing it
        let d = [0x05, 0x01, 0x09, 0x04, 0xA1, 0x01];
        assert!(matches!(
            parse_descriptor(&d),
            Err(DescriptorError::UnclosedCollection { count: 1 })
        ));
    }

    #[test]
    fn simple_2_axis_joystick() {
        let d = simple_joystick_descriptor(2, 0);
        let desc = parse_descriptor(&d).unwrap();

        assert_eq!(desc.axis_count(), 2);
        assert_eq!(desc.button_count(), 0);
        assert_eq!(desc.hat_count(), 0);
        assert_eq!(desc.report_size_bits, 32); // 2 axes × 16 bits
    }

    #[test]
    fn simple_joystick_with_buttons() {
        let d = simple_joystick_descriptor(3, 8);
        let desc = parse_descriptor(&d).unwrap();

        assert_eq!(desc.axis_count(), 3);
        assert_eq!(desc.button_count(), 8);
        assert_eq!(desc.hat_count(), 0);
        // 3 × 16 bits + 8 × 1 bit = 56 bits
        assert_eq!(desc.report_size_bits, 56);
    }

    #[test]
    fn hotas_axes_buttons_hats() {
        let d = hotas_descriptor(6, 32, 1);
        let desc = parse_descriptor(&d).unwrap();

        assert_eq!(desc.axis_count(), 6);
        assert_eq!(desc.button_count(), 32);
        assert_eq!(desc.hat_count(), 1);
    }

    #[test]
    fn hotas_two_hats() {
        let d = hotas_descriptor(4, 12, 2);
        let desc = parse_descriptor(&d).unwrap();

        assert_eq!(desc.axis_count(), 4);
        assert_eq!(desc.button_count(), 12);
        assert_eq!(desc.hat_count(), 2);
    }

    #[test]
    fn axis_ranges_match_descriptor() {
        let d = simple_joystick_descriptor(3, 0);
        let desc = parse_descriptor(&d).unwrap();
        let ranges = desc.axis_ranges();

        assert_eq!(ranges.len(), 3);
        for &(lo, hi) in &ranges {
            assert_eq!(lo, 0);
            assert_eq!(hi, 1023);
        }
    }

    #[test]
    fn collection_structure() {
        let d = simple_joystick_descriptor(2, 4);
        let desc = parse_descriptor(&d).unwrap();

        assert_eq!(desc.collections.len(), 1);
        let col = &desc.collections[0];
        assert_eq!(col.collection_type, CollectionType::Application);
        assert_eq!(col.usage_page, USAGE_PAGE_GENERIC_DESKTOP);
        assert_eq!(col.usage, 0x04); // Joystick
    }

    #[test]
    fn display_impl() {
        let d = simple_joystick_descriptor(2, 4);
        let desc = parse_descriptor(&d).unwrap();
        let s = format!("{desc}");
        assert!(s.contains("axes: 2"));
        assert!(s.contains("buttons: 4"));
    }

    #[test]
    fn signed_logical_min() {
        // Build a descriptor with negative logical min (e.g. -128..127 for an axis)
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]); // collection
        d.extend_from_slice(&[0x05, 0x01]); // Usage Page (Generic Desktop)
        d.extend_from_slice(&[0x09, 0x30]); // Usage (X)
        d.extend_from_slice(&[0x15, 0x80]); // Logical Minimum (-128)
        d.extend_from_slice(&[0x25, 0x7F]); // Logical Maximum (127)
        d.extend_from_slice(&[0x75, 0x08]); // Report Size (8)
        d.extend_from_slice(&[0x95, 0x01]); // Report Count (1)
        d.extend_from_slice(&[0x81, 0x02]); // Input
        d.push(0xC0); // End Collection

        let desc = parse_descriptor(&d).unwrap();
        assert_eq!(desc.axis_count(), 1);
        let ranges = desc.axis_ranges();
        assert_eq!(ranges[0], (-128, 127));
    }

    #[test]
    fn long_item_is_skipped() {
        // A long item (0xFE) with 2 bytes of data, embedded in a valid descriptor
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]); // collection
        // Long item: prefix=0xFE, length=2, tag=0x10, data=[0xAA, 0xBB]
        d.extend_from_slice(&[0xFE, 0x02, 0x10, 0xAA, 0xBB]);
        d.push(0xC0); // End Collection

        let desc = parse_descriptor(&d).unwrap();
        assert_eq!(desc.collections.len(), 1);
    }

    #[test]
    fn full_hotas_with_output() {
        // Descriptor with input axes AND an output item (LED)
        let mut d = Vec::new();
        d.extend_from_slice(&[0x05, 0x01, 0x09, 0x04, 0xA1, 0x01]);

        // 2 input axes
        d.extend_from_slice(&[0x05, 0x01, 0x19, 0x30, 0x29, 0x31]);
        d.extend_from_slice(&[0x15, 0x00, 0x26, 0xFF, 0x03]);
        d.extend_from_slice(&[0x75, 0x10, 0x95, 0x02, 0x81, 0x02]);

        // 1 output byte (e.g. LED control)
        d.extend_from_slice(&[0x05, 0x08]); // Usage Page (LEDs)
        d.extend_from_slice(&[0x09, 0x01]); // Usage (Num Lock)
        d.extend_from_slice(&[0x75, 0x08, 0x95, 0x01, 0x91, 0x02]); // Output

        d.push(0xC0);

        let desc = parse_descriptor(&d).unwrap();
        assert_eq!(desc.axis_count(), 2);
        // The output item should be in the collection
        let output_items: Vec<_> = desc.collections[0]
            .items
            .iter()
            .filter(|i| i.item_type == HidItemType::Output)
            .collect();
        assert_eq!(output_items.len(), 1);
    }
}
