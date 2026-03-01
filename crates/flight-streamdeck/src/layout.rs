// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Page / folder navigation for StreamDeck layouts
//!
//! Organises StreamDeck keys into multiple pages (folders) that the user can
//! navigate with dedicated "next page" / "previous page" / "back" keys.
//! Supports profile loading per aircraft type.

use crate::actions::ActionTemplate;
use crate::device::StreamDeckModel;
use crate::profiles::AircraftType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info};

/// Errors during layout operations.
#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("Page index {index} out of range (0..{page_count})")]
    PageOutOfRange { index: usize, page_count: usize },
    #[error("Key position ({row}, {col}) out of range for {model:?}")]
    KeyOutOfRange {
        row: u8,
        col: u8,
        model: StreamDeckModel,
    },
    #[error("No grid layout for model {0:?}")]
    NoGrid(StreamDeckModel),
}

/// A single key slot in a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeySlot {
    pub row: u8,
    pub col: u8,
    /// Bound action template id, or `None` if the slot is empty.
    pub action_id: Option<String>,
    /// Navigation target if this key is a folder/navigation key.
    pub nav_target: Option<NavTarget>,
}

/// Where a navigation key takes the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NavTarget {
    /// Go to an absolute page index.
    Page(usize),
    /// Go to the next page.
    NextPage,
    /// Go to the previous page.
    PrevPage,
    /// Go back to the parent folder (root page).
    Back,
}

/// A single page of keys.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub name: String,
    pub keys: Vec<KeySlot>,
}

impl Page {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            keys: Vec::new(),
        }
    }

    /// Add a key slot.
    pub fn add_key(&mut self, slot: KeySlot) {
        self.keys.push(slot);
    }

    /// Get key at grid position.
    pub fn key_at(&self, row: u8, col: u8) -> Option<&KeySlot> {
        self.keys.iter().find(|k| k.row == row && k.col == col)
    }
}

/// Multi-page layout for a StreamDeck device.
pub struct PageLayout {
    model: StreamDeckModel,
    pages: Vec<Page>,
    current_page: usize,
}

impl PageLayout {
    /// Create a new layout for the given device model.
    pub fn new(model: StreamDeckModel) -> Result<Self, LayoutError> {
        if model.grid_layout().is_none() {
            return Err(LayoutError::NoGrid(model));
        }
        Ok(Self {
            model,
            pages: Vec::new(),
            current_page: 0,
        })
    }

    /// Add a page and return its index.
    pub fn add_page(&mut self, page: Page) -> usize {
        self.pages.push(page);
        self.pages.len() - 1
    }

    /// Navigate to a specific page.
    pub fn go_to_page(&mut self, index: usize) -> Result<&Page, LayoutError> {
        if index >= self.pages.len() {
            return Err(LayoutError::PageOutOfRange {
                index,
                page_count: self.pages.len(),
            });
        }
        self.current_page = index;
        debug!("Navigated to page {} ({})", index, self.pages[index].name);
        Ok(&self.pages[index])
    }

    /// Navigate forward one page (wraps).
    pub fn next_page(&mut self) -> &Page {
        if !self.pages.is_empty() {
            self.current_page = (self.current_page + 1) % self.pages.len();
        }
        &self.pages[self.current_page]
    }

    /// Navigate back one page (wraps).
    pub fn prev_page(&mut self) -> &Page {
        if !self.pages.is_empty() {
            self.current_page = if self.current_page == 0 {
                self.pages.len() - 1
            } else {
                self.current_page - 1
            };
        }
        &self.pages[self.current_page]
    }

    /// Get the current page.
    pub fn current(&self) -> Option<&Page> {
        self.pages.get(self.current_page)
    }

    /// Current page index.
    pub fn current_index(&self) -> usize {
        self.current_page
    }

    /// Total pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// Device model.
    pub fn model(&self) -> StreamDeckModel {
        self.model
    }
}

/// Builds a profile-based layout for a given aircraft type and device model.
pub struct ProfileLayout;

impl ProfileLayout {
    /// Build a default multi-page layout for the given aircraft + device.
    pub fn build(
        aircraft: AircraftType,
        model: StreamDeckModel,
        templates: &HashMap<String, ActionTemplate>,
    ) -> Result<PageLayout, LayoutError> {
        let (rows, cols) = model.grid_layout().ok_or(LayoutError::NoGrid(model))?;

        // Reserve last key on each page for navigation.
        let usable = (rows as usize) * (cols as usize) - 1;

        // Collect templates in category priority order for this aircraft.
        let categories = Self::category_order(aircraft);
        let mut ordered_actions: Vec<&ActionTemplate> = Vec::new();
        for cat in &categories {
            let mut cat_actions: Vec<&ActionTemplate> =
                templates.values().filter(|t| &t.category == cat).collect();
            cat_actions.sort_by(|a, b| a.id.cmp(&b.id));
            ordered_actions.extend(cat_actions);
        }

        let mut layout = PageLayout::new(model)?;
        let chunks: Vec<Vec<&ActionTemplate>> =
            ordered_actions.chunks(usable).map(|c| c.to_vec()).collect();

        for (page_idx, chunk) in chunks.iter().enumerate() {
            let mut page = Page::new(&format!("{aircraft:?} - Page {}", page_idx + 1));

            for (i, action) in chunk.iter().enumerate() {
                let r = (i / cols as usize) as u8;
                let c = (i % cols as usize) as u8;
                page.add_key(KeySlot {
                    row: r,
                    col: c,
                    action_id: Some(action.id.clone()),
                    nav_target: None,
                });
            }

            // Add navigation key in the last position.
            let nav_row = rows - 1;
            let nav_col = cols - 1;
            page.add_key(KeySlot {
                row: nav_row,
                col: nav_col,
                action_id: None,
                nav_target: Some(NavTarget::NextPage),
            });

            layout.add_page(page);
        }

        // If no templates matched, add an empty root page.
        if layout.page_count() == 0 {
            layout.add_page(Page::new(&format!("{aircraft:?} - Main")));
        }

        info!(
            "Built {}-page layout for {:?} on {}",
            layout.page_count(),
            aircraft,
            model.display_name()
        );
        Ok(layout)
    }

    /// Preferred category ordering per aircraft type.
    fn category_order(aircraft: AircraftType) -> Vec<crate::actions::ActionCategory> {
        use crate::actions::ActionCategory;
        match aircraft {
            AircraftType::GA => vec![
                ActionCategory::Lights,
                ActionCategory::Systems,
                ActionCategory::Navigation,
                ActionCategory::Communication,
                ActionCategory::Autopilot,
            ],
            AircraftType::Airbus => vec![
                ActionCategory::Autopilot,
                ActionCategory::Navigation,
                ActionCategory::Communication,
                ActionCategory::Lights,
                ActionCategory::Systems,
            ],
            AircraftType::Helo => vec![
                ActionCategory::Systems,
                ActionCategory::Lights,
                ActionCategory::Navigation,
                ActionCategory::Communication,
                ActionCategory::Autopilot,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::builtin_templates;

    // ── Page basics ────────────────────────────────────────────────────

    #[test]
    fn test_page_new() {
        let page = Page::new("Test");
        assert_eq!(page.name, "Test");
        assert!(page.keys.is_empty());
    }

    #[test]
    fn test_page_add_and_lookup() {
        let mut page = Page::new("P1");
        page.add_key(KeySlot {
            row: 0,
            col: 1,
            action_id: Some("act-1".into()),
            nav_target: None,
        });
        assert!(page.key_at(0, 1).is_some());
        assert!(page.key_at(0, 0).is_none());
    }

    // ── PageLayout navigation ──────────────────────────────────────────

    #[test]
    fn test_layout_creation() {
        let layout = PageLayout::new(StreamDeckModel::Original).unwrap();
        assert_eq!(layout.page_count(), 0);
        assert_eq!(layout.model(), StreamDeckModel::Original);
    }

    #[test]
    fn test_layout_no_grid() {
        assert!(PageLayout::new(StreamDeckModel::Pedal).is_err());
    }

    #[test]
    fn test_page_navigation_forward() {
        let mut layout = PageLayout::new(StreamDeckModel::Mini).unwrap();
        layout.add_page(Page::new("A"));
        layout.add_page(Page::new("B"));
        layout.add_page(Page::new("C"));

        assert_eq!(layout.current_index(), 0);
        layout.next_page();
        assert_eq!(layout.current_index(), 1);
        layout.next_page();
        assert_eq!(layout.current_index(), 2);
        // Wrap around
        layout.next_page();
        assert_eq!(layout.current_index(), 0);
    }

    #[test]
    fn test_page_navigation_backward() {
        let mut layout = PageLayout::new(StreamDeckModel::Mini).unwrap();
        layout.add_page(Page::new("A"));
        layout.add_page(Page::new("B"));

        // At page 0, going back should wrap to last page.
        layout.prev_page();
        assert_eq!(layout.current_index(), 1);
        layout.prev_page();
        assert_eq!(layout.current_index(), 0);
    }

    #[test]
    fn test_go_to_page() {
        let mut layout = PageLayout::new(StreamDeckModel::Xl).unwrap();
        layout.add_page(Page::new("A"));
        layout.add_page(Page::new("B"));

        assert!(layout.go_to_page(1).is_ok());
        assert_eq!(layout.current_index(), 1);

        assert!(layout.go_to_page(5).is_err());
    }

    #[test]
    fn test_current_page() {
        let mut layout = PageLayout::new(StreamDeckModel::Original).unwrap();
        assert!(layout.current().is_none());

        layout.add_page(Page::new("Root"));
        assert_eq!(layout.current().unwrap().name, "Root");
    }

    // ── ProfileLayout builder ──────────────────────────────────────────

    #[test]
    fn test_profile_layout_ga() {
        let templates: HashMap<String, ActionTemplate> = builtin_templates()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();

        let layout =
            ProfileLayout::build(AircraftType::GA, StreamDeckModel::Original, &templates).unwrap();

        assert!(layout.page_count() >= 1);
        // First page should have keys
        let first = &layout.pages[0];
        assert!(!first.keys.is_empty());
        // Last key should be a nav key
        let last = first.keys.last().unwrap();
        assert!(last.nav_target.is_some());
    }

    #[test]
    fn test_profile_layout_airbus_xl() {
        let templates: HashMap<String, ActionTemplate> = builtin_templates()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();

        let layout =
            ProfileLayout::build(AircraftType::Airbus, StreamDeckModel::Xl, &templates).unwrap();
        // XL has 32 keys so 31 usable; 21 actions fits in 1 page.
        assert!(layout.page_count() >= 1);
    }

    #[test]
    fn test_profile_layout_helo_mini() {
        let templates: HashMap<String, ActionTemplate> = builtin_templates()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();

        let layout =
            ProfileLayout::build(AircraftType::Helo, StreamDeckModel::Mini, &templates).unwrap();
        // Mini has 6 keys, 5 usable per page, 21 actions → multiple pages.
        assert!(layout.page_count() > 1);
    }

    #[test]
    fn test_profile_layout_empty_templates() {
        let templates: HashMap<String, ActionTemplate> = HashMap::new();
        let layout =
            ProfileLayout::build(AircraftType::GA, StreamDeckModel::Original, &templates).unwrap();
        // Should still have at least one root page.
        assert_eq!(layout.page_count(), 1);
    }

    #[test]
    fn test_profile_layout_pedal_fails() {
        let templates: HashMap<String, ActionTemplate> = HashMap::new();
        assert!(
            ProfileLayout::build(AircraftType::GA, StreamDeckModel::Pedal, &templates).is_err()
        );
    }

    #[test]
    fn test_profile_layout_mk2() {
        let templates: HashMap<String, ActionTemplate> = builtin_templates()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();

        let layout =
            ProfileLayout::build(AircraftType::GA, StreamDeckModel::Mk2, &templates).unwrap();
        assert!(layout.page_count() >= 1);
        // MK.2 has same layout as Original (15 keys)
        let first = &layout.pages[0];
        assert!(!first.keys.is_empty());
    }

    #[test]
    fn test_page_navigation_go_and_back() {
        let mut layout = PageLayout::new(StreamDeckModel::Xl).unwrap();
        layout.add_page(Page::new("A"));
        layout.add_page(Page::new("B"));
        layout.add_page(Page::new("C"));

        layout.go_to_page(2).unwrap();
        assert_eq!(layout.current_index(), 2);
        layout.prev_page();
        assert_eq!(layout.current_index(), 1);
    }

    #[test]
    fn test_key_slot_nav_target() {
        let slot = KeySlot {
            row: 0,
            col: 0,
            action_id: None,
            nav_target: Some(NavTarget::NextPage),
        };
        assert!(slot.action_id.is_none());
        assert!(slot.nav_target.is_some());
    }

    #[test]
    fn test_all_aircraft_all_models() {
        let templates: HashMap<String, ActionTemplate> = builtin_templates()
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();

        for aircraft in &[AircraftType::GA, AircraftType::Airbus, AircraftType::Helo] {
            for model in StreamDeckModel::all() {
                let result = ProfileLayout::build(*aircraft, *model, &templates);
                if model.grid_layout().is_some() {
                    assert!(result.is_ok(), "{:?} + {:?} should build", aircraft, model);
                    assert!(result.unwrap().page_count() >= 1);
                } else {
                    assert!(result.is_err(), "{:?} + {:?} should fail (no grid)", aircraft, model);
                }
            }
        }
    }
}
