// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Minimal HID report descriptor parsing utilities.
//!
//! This parser intentionally focuses on extracting Usage Page / Usage pairs
//! for basic device identification and axis-mode quirks. It is not a full
//! HID descriptor implementation.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidUsage {
    pub usage_page: u16,
    pub usage: u16,
}

const HID_ITEM_LONG: u8 = 0xFE;
const MAIN_TAG_INPUT: u8 = 0x08;
const MAIN_TAG_OUTPUT: u8 = 0x09;
const MAIN_TAG_COLLECTION: u8 = 0x0A;
const MAIN_TAG_FEATURE: u8 = 0x0B;

const GLOBAL_TAG_USAGE_PAGE: u8 = 0x00;

const LOCAL_TAG_USAGE: u8 = 0x00;
const LOCAL_TAG_USAGE_MIN: u8 = 0x01;
const LOCAL_TAG_USAGE_MAX: u8 = 0x02;

fn item_size(size_code: u8) -> usize {
    match size_code {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    }
}

fn read_value(data: &[u8]) -> u32 {
    let mut value = 0u32;
    for (shift, byte) in data.iter().enumerate() {
        value |= (*byte as u32) << (shift * 8);
    }
    value
}

fn push_usages(
    usages: &mut Vec<HidUsage>,
    usage_page: u16,
    local_usages: &mut Vec<u16>,
    usage_min: &mut Option<u16>,
    usage_max: &mut Option<u16>,
) {
    if let (Some(min), Some(max)) = (*usage_min, *usage_max) {
        let (start, end) = if min <= max { (min, max) } else { (max, min) };
        let count = end.saturating_sub(start) + 1;
        if count <= 64 {
            for usage in start..=end {
                usages.push(HidUsage { usage_page, usage });
            }
        }
    } else {
        for usage in local_usages.iter().copied() {
            usages.push(HidUsage { usage_page, usage });
        }
    }

    local_usages.clear();
    *usage_min = None;
    *usage_max = None;
}

/// Extract Usage Page / Usage pairs from a HID report descriptor.
///
/// This function focuses on usages associated with Input/Output/Feature items.
pub fn extract_usages(descriptor: &[u8]) -> Vec<HidUsage> {
    let mut usages = Vec::new();
    let mut usage_page: u16 = 0;
    let mut local_usages: Vec<u16> = Vec::new();
    let mut usage_min: Option<u16> = None;
    let mut usage_max: Option<u16> = None;

    let mut idx = 0usize;
    while idx < descriptor.len() {
        let prefix = descriptor[idx];
        idx += 1;

        if prefix == HID_ITEM_LONG {
            if idx + 1 >= descriptor.len() {
                break;
            }
            let data_len = descriptor[idx] as usize;
            idx += 2; // skip length + long item tag
            idx = idx.saturating_add(data_len);
            continue;
        }

        let size_code = prefix & 0x03;
        let item_type = (prefix >> 2) & 0x03;
        let tag = (prefix >> 4) & 0x0F;
        let size = item_size(size_code);

        if idx + size > descriptor.len() {
            break;
        }

        let value = read_value(&descriptor[idx..idx + size]);
        idx += size;

        match item_type {
            0x00 => {
                // Main
                match tag {
                    MAIN_TAG_INPUT | MAIN_TAG_OUTPUT | MAIN_TAG_FEATURE => {
                        push_usages(
                            &mut usages,
                            usage_page,
                            &mut local_usages,
                            &mut usage_min,
                            &mut usage_max,
                        );
                    }
                    MAIN_TAG_COLLECTION => {
                        // Usage is consumed by Collection; clear locals.
                        local_usages.clear();
                        usage_min = None;
                        usage_max = None;
                    }
                    _ => {}
                }
            }
            0x01 => {
                // Global
                if tag == GLOBAL_TAG_USAGE_PAGE {
                    usage_page = value as u16;
                }
            }
            0x02 => {
                // Local
                match tag {
                    LOCAL_TAG_USAGE => local_usages.push(value as u16),
                    LOCAL_TAG_USAGE_MIN => usage_min = Some(value as u16),
                    LOCAL_TAG_USAGE_MAX => usage_max = Some(value as u16),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    usages
}
