// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

use crate::DescriptorError;

/// Main item tags.
pub(super) const MAIN_INPUT: u8 = 0x08;
pub(super) const MAIN_OUTPUT: u8 = 0x09;
pub(super) const MAIN_FEATURE: u8 = 0x0B;
pub(super) const MAIN_COLLECTION: u8 = 0x0A;
pub(super) const MAIN_END_COLLECTION: u8 = 0x0C;

/// Global item tags.
pub(super) const GLOBAL_USAGE_PAGE: u8 = 0x00;
pub(super) const GLOBAL_LOGICAL_MIN: u8 = 0x01;
pub(super) const GLOBAL_LOGICAL_MAX: u8 = 0x02;
pub(super) const GLOBAL_PHYSICAL_MIN: u8 = 0x03;
pub(super) const GLOBAL_PHYSICAL_MAX: u8 = 0x04;
pub(super) const GLOBAL_REPORT_SIZE: u8 = 0x07;
pub(super) const GLOBAL_REPORT_COUNT: u8 = 0x09;
pub(super) const GLOBAL_REPORT_ID: u8 = 0x08;

/// Local item tags.
pub(super) const LOCAL_USAGE: u8 = 0x00;
pub(super) const LOCAL_USAGE_MIN: u8 = 0x01;
pub(super) const LOCAL_USAGE_MAX: u8 = 0x02;

const HID_ITEM_LONG: u8 = 0xFE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ItemType {
    Main,
    Global,
    Local,
    Reserved,
}

pub(super) struct ShortItem<'a> {
    pub item_type: ItemType,
    pub tag: u8,
    pub data: &'a [u8],
    pub offset: usize,
}

/// Decode the next short item and advance `idx`.
///
/// Long HID items are descriptor metadata and do not affect the parser state
/// used by OpenFlight, so they are consumed here and represented as `None`.
pub(super) fn next_short_item<'a>(
    bytes: &'a [u8],
    idx: &mut usize,
) -> Result<Option<ShortItem<'a>>, DescriptorError> {
    let prefix = bytes[*idx];
    let offset = *idx;
    *idx += 1;

    if prefix == HID_ITEM_LONG {
        skip_long_item(bytes, idx, offset)?;
        return Ok(None);
    }

    let size = item_size(prefix & 0x03);
    if *idx + size > bytes.len() {
        return Err(DescriptorError::Truncated { offset });
    }

    let data = &bytes[*idx..*idx + size];
    *idx += size;

    Ok(Some(ShortItem {
        item_type: item_type((prefix >> 2) & 0x03),
        tag: (prefix >> 4) & 0x0F,
        data,
        offset,
    }))
}

fn skip_long_item(bytes: &[u8], idx: &mut usize, offset: usize) -> Result<(), DescriptorError> {
    if *idx >= bytes.len() {
        return Err(DescriptorError::Truncated { offset });
    }

    let data_len = bytes[*idx] as usize;
    *idx += 2;
    if (*idx).saturating_add(data_len) > bytes.len() {
        return Err(DescriptorError::Truncated { offset });
    }

    *idx += data_len;
    Ok(())
}

fn item_size(size_code: u8) -> usize {
    match size_code {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 4,
    }
}

fn item_type(type_code: u8) -> ItemType {
    match type_code {
        0x00 => ItemType::Main,
        0x01 => ItemType::Global,
        0x02 => ItemType::Local,
        _ => ItemType::Reserved,
    }
}

/// Read a little-endian unsigned value from `data` (0–4 bytes).
pub(super) fn read_unsigned(data: &[u8]) -> u32 {
    let mut v = 0u32;
    for (i, &b) in data.iter().enumerate() {
        v |= (b as u32) << (i * 8);
    }
    v
}

/// Read a little-endian *signed* value from `data` (0–4 bytes).
pub(super) fn read_signed(data: &[u8]) -> i32 {
    if data.is_empty() {
        return 0;
    }
    let unsigned = read_unsigned(data);
    let bits = data.len() * 8;
    if bits < 32 && (unsigned >> (bits - 1)) & 1 == 1 {
        (unsigned | (!0u32 << bits)) as i32
    } else {
        unsigned as i32
    }
}
