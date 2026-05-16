// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

#[derive(Clone, Default)]
pub(super) struct GlobalState {
    pub usage_page: u16,
    pub logical_min: i32,
    pub logical_max: i32,
    pub physical_min: i32,
    pub physical_max: i32,
    pub report_size: u32,
    pub report_count: u32,
    pub report_id: Option<u8>,
}

#[derive(Default)]
pub(super) struct LocalState {
    pub usages: Vec<u16>,
    pub usage_min: Option<u16>,
    pub usage_max: Option<u16>,
}

impl LocalState {
    pub fn clear(&mut self) {
        self.usages.clear();
        self.usage_min = None;
        self.usage_max = None;
    }

    pub fn expanded_usages(&self) -> Vec<u16> {
        if let (Some(min), Some(max)) = (self.usage_min, self.usage_max) {
            let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
            (lo..=hi).collect()
        } else {
            self.usages.clone()
        }
    }
}
