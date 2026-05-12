// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

mod item;
mod state;
mod stats;

use crate::{
    CollectionType, DescriptorError, HidCollection, MainItemFlags, ReportDescriptor, ReportField,
    ReportType,
};
use item::{
    GLOBAL_LOGICAL_MAX, GLOBAL_LOGICAL_MIN, GLOBAL_PHYSICAL_MAX, GLOBAL_PHYSICAL_MIN,
    GLOBAL_REPORT_COUNT, GLOBAL_REPORT_ID, GLOBAL_REPORT_SIZE, GLOBAL_USAGE_PAGE, ItemType,
    LOCAL_USAGE, LOCAL_USAGE_MAX, LOCAL_USAGE_MIN, MAIN_COLLECTION, MAIN_END_COLLECTION,
    MAIN_FEATURE, MAIN_INPUT, MAIN_OUTPUT, ShortItem, next_short_item, read_signed, read_unsigned,
};
use state::{GlobalState, LocalState};
use stats::DescriptorStats;

struct ParserState {
    global: GlobalState,
    local: LocalState,
    global_stack: Vec<GlobalState>,
    collection_stack: Vec<HidCollection>,
    finished_collections: Vec<HidCollection>,
    stats: DescriptorStats,
}

impl ParserState {
    fn new() -> Self {
        Self {
            global: GlobalState::default(),
            local: LocalState::default(),
            global_stack: Vec::new(),
            collection_stack: Vec::new(),
            finished_collections: Vec::new(),
            stats: DescriptorStats::default(),
        }
    }

    fn handle_item(&mut self, item: ShortItem<'_>) -> Result<(), DescriptorError> {
        match item.item_type {
            ItemType::Main => self.handle_main_item(item),
            ItemType::Global => {
                self.handle_global_item(item);
                Ok(())
            }
            ItemType::Local => {
                self.handle_local_item(item);
                Ok(())
            }
            ItemType::Reserved => Ok(()),
        }
    }

    fn handle_main_item(&mut self, item: ShortItem<'_>) -> Result<(), DescriptorError> {
        match item.tag {
            MAIN_INPUT | MAIN_OUTPUT | MAIN_FEATURE => self.handle_field_item(item),
            MAIN_COLLECTION => {
                self.start_collection(item.data);
                Ok(())
            }
            MAIN_END_COLLECTION => self.end_collection(item.offset),
            _ => {
                self.local.clear();
                Ok(())
            }
        }
    }

    fn handle_field_item(&mut self, item: ShortItem<'_>) -> Result<(), DescriptorError> {
        let report_type = match item.tag {
            MAIN_INPUT => ReportType::Input,
            MAIN_OUTPUT => ReportType::Output,
            _ => ReportType::Feature,
        };
        let field = self.field_from_current_state(report_type, item.data);

        self.stats.account_field(&field);
        if let Some(collection) = self.collection_stack.last_mut() {
            collection.fields.push(field);
        }
        self.local.clear();
        Ok(())
    }

    fn field_from_current_state(&self, report_type: ReportType, data: &[u8]) -> ReportField {
        let usages = self.local.expanded_usages();
        let primary_usage = usages.first().copied().unwrap_or(0);

        ReportField {
            report_type,
            flags: MainItemFlags(read_unsigned(data)),
            usage_page: self.global.usage_page,
            usage: primary_usage,
            logical_min: self.global.logical_min,
            logical_max: self.global.logical_max,
            physical_min: self.global.physical_min,
            physical_max: self.global.physical_max,
            report_size: self.global.report_size,
            report_count: self.global.report_count,
            report_id: self.global.report_id,
        }
    }

    fn start_collection(&mut self, data: &[u8]) {
        let primary = self.local.expanded_usages();
        let usage = primary.first().copied().unwrap_or(0);
        self.collection_stack.push(HidCollection {
            usage_page: self.global.usage_page,
            usage,
            collection_type: CollectionType::from_value(read_unsigned(data)),
            fields: Vec::new(),
        });
        self.local.clear();
    }

    fn end_collection(&mut self, offset: usize) -> Result<(), DescriptorError> {
        let collection = self
            .collection_stack
            .pop()
            .ok_or(DescriptorError::UnmatchedEnd { offset })?;

        if self.collection_stack.is_empty() {
            self.finished_collections.push(collection);
        } else if let Some(parent) = self.collection_stack.last_mut() {
            parent.fields.extend(collection.fields);
        }

        self.local.clear();
        Ok(())
    }

    fn handle_global_item(&mut self, item: ShortItem<'_>) {
        match item.tag {
            GLOBAL_USAGE_PAGE => self.global.usage_page = read_unsigned(item.data) as u16,
            GLOBAL_LOGICAL_MIN => self.global.logical_min = read_signed(item.data),
            GLOBAL_LOGICAL_MAX => self.global.logical_max = read_signed(item.data),
            GLOBAL_PHYSICAL_MIN => self.global.physical_min = read_signed(item.data),
            GLOBAL_PHYSICAL_MAX => self.global.physical_max = read_signed(item.data),
            GLOBAL_REPORT_SIZE => self.global.report_size = read_unsigned(item.data),
            GLOBAL_REPORT_COUNT => self.global.report_count = read_unsigned(item.data),
            GLOBAL_REPORT_ID => self.global.report_id = Some(read_unsigned(item.data) as u8),
            0x0A => self.global_stack.push(self.global.clone()),
            0x0B => {
                if let Some(global) = self.global_stack.pop() {
                    self.global = global;
                }
            }
            _ => {}
        }
    }

    fn handle_local_item(&mut self, item: ShortItem<'_>) {
        match item.tag {
            LOCAL_USAGE => self.local.usages.push(read_unsigned(item.data) as u16),
            LOCAL_USAGE_MIN => self.local.usage_min = Some(read_unsigned(item.data) as u16),
            LOCAL_USAGE_MAX => self.local.usage_max = Some(read_unsigned(item.data) as u16),
            _ => {}
        }
    }

    fn finish(self) -> Result<ReportDescriptor, DescriptorError> {
        if !self.collection_stack.is_empty() {
            return Err(DescriptorError::UnclosedCollection {
                count: self.collection_stack.len(),
            });
        }

        Ok(ReportDescriptor {
            collections: self.finished_collections,
            total_axes: self.stats.total_axes,
            total_buttons: self.stats.total_buttons,
            total_hats: self.stats.total_hats,
            report_size_bits: self.stats.report_bits,
        })
    }
}

/// Parse a raw HID report descriptor into a structured [`ReportDescriptor`].
pub fn parse_descriptor(bytes: &[u8]) -> Result<ReportDescriptor, DescriptorError> {
    if bytes.is_empty() {
        return Err(DescriptorError::Empty);
    }

    let mut parser = ParserState::new();
    let mut idx = 0usize;
    while idx < bytes.len() {
        if let Some(item) = next_short_item(bytes, &mut idx)? {
            parser.handle_item(item)?;
        }
    }

    parser.finish()
}
