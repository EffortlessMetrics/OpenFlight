// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

use crate::{ReportField, ReportType};

#[derive(Default)]
pub(super) struct DescriptorStats {
    pub total_axes: u32,
    pub total_buttons: u32,
    pub total_hats: u32,
    pub report_bits: u32,
}

impl DescriptorStats {
    pub fn account_field(&mut self, field: &ReportField) {
        if field.report_type != ReportType::Input {
            return;
        }

        if !field.flags.is_constant() {
            if field.is_button() {
                self.total_buttons = self.total_buttons.saturating_add(field.report_count);
            } else if field.is_hat() {
                self.total_hats = self.total_hats.saturating_add(field.report_count);
            } else if field.is_axis() {
                self.total_axes = self.total_axes.saturating_add(field.report_count);
            }
        }

        self.report_bits = self
            .report_bits
            .saturating_add(field.report_size.saturating_mul(field.report_count));
    }
}
