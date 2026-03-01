// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Multi-page navigation system for StreamDeck devices
//!
//! Organises [`ButtonConfig`] items into named pages with forward/backward
//! navigation and a page stack for temporary overlays (e.g. pop-up sub-menus).

use crate::button_manager::ButtonConfig;
use std::collections::HashMap;
use thiserror::Error;

// ── Errors ───────────────────────────────────────────────────────────────────

/// Errors from page navigation operations.
#[derive(Debug, Error)]
pub enum PageError {
    #[error("Page not found: {0}")]
    NotFound(String),
    #[error("Page stack is empty — nothing to pop")]
    StackEmpty,
    #[error("No pages registered")]
    NoPages,
}

// ── ButtonPage ───────────────────────────────────────────────────────────────

/// A named page of button configurations.
#[derive(Debug, Clone)]
pub struct ButtonPage {
    /// Human-readable page name.
    pub name: String,
    /// Button configs indexed by slot position.
    pub buttons: Vec<ButtonConfig>,
}

impl ButtonPage {
    /// Create an empty page with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            buttons: Vec::new(),
        }
    }

    /// Create a page pre-populated with button configs.
    pub fn with_buttons(name: &str, buttons: Vec<ButtonConfig>) -> Self {
        Self {
            name: name.to_string(),
            buttons,
        }
    }
}

// ── PageManager ──────────────────────────────────────────────────────────────

/// Manages a set of named pages with linear navigation and an overlay stack.
pub struct PageManager {
    /// All registered pages keyed by name.
    pages: HashMap<String, ButtonPage>,
    /// Ordered page names for next/prev navigation.
    page_order: Vec<String>,
    /// Index into `page_order` for the current page.
    current_index: usize,
    /// Stack of pushed overlay page names (most recent on top).
    stack: Vec<String>,
}

impl PageManager {
    /// Create an empty page manager.
    pub fn new() -> Self {
        Self {
            pages: HashMap::new(),
            page_order: Vec::new(),
            current_index: 0,
            stack: Vec::new(),
        }
    }

    /// Register a page. The insertion order determines next/prev sequence.
    pub fn add_page(&mut self, page: ButtonPage) {
        let name = page.name.clone();
        self.pages.insert(name.clone(), page);
        if !self.page_order.contains(&name) {
            self.page_order.push(name);
        }
    }

    /// Switch to a page by name.
    pub fn switch_page(&mut self, name: &str) -> Result<(), PageError> {
        let idx = self
            .page_order
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| PageError::NotFound(name.to_string()))?;
        self.current_index = idx;
        Ok(())
    }

    /// Navigate to the next page (wraps around).
    pub fn next_page(&mut self) -> Result<&ButtonPage, PageError> {
        if self.page_order.is_empty() {
            return Err(PageError::NoPages);
        }
        self.current_index = (self.current_index + 1) % self.page_order.len();
        self.current_page()
    }

    /// Navigate to the previous page (wraps around).
    pub fn prev_page(&mut self) -> Result<&ButtonPage, PageError> {
        if self.page_order.is_empty() {
            return Err(PageError::NoPages);
        }
        self.current_index = if self.current_index == 0 {
            self.page_order.len() - 1
        } else {
            self.current_index - 1
        };
        self.current_page()
    }

    /// Push an overlay page onto the stack (does not change base navigation).
    pub fn push_page(&mut self, name: &str) -> Result<(), PageError> {
        if !self.pages.contains_key(name) {
            return Err(PageError::NotFound(name.to_string()));
        }
        self.stack.push(name.to_string());
        Ok(())
    }

    /// Pop the top overlay page from the stack.
    pub fn pop_page(&mut self) -> Result<String, PageError> {
        self.stack.pop().ok_or(PageError::StackEmpty)
    }

    /// Get the active page — top of overlay stack if non-empty, otherwise the
    /// current base page.
    pub fn current_page(&self) -> Result<&ButtonPage, PageError> {
        let name = if let Some(top) = self.stack.last() {
            top.as_str()
        } else {
            self.page_order
                .get(self.current_index)
                .map(String::as_str)
                .ok_or(PageError::NoPages)?
        };
        self.pages
            .get(name)
            .ok_or_else(|| PageError::NotFound(name.to_string()))
    }

    /// Convenience: get the button configs of the active page.
    pub fn current_buttons(&self) -> Result<&[ButtonConfig], PageError> {
        self.current_page().map(|p| p.buttons.as_slice())
    }

    /// Current base page index.
    pub fn current_index(&self) -> usize {
        self.current_index
    }

    /// Number of registered pages.
    pub fn page_count(&self) -> usize {
        self.page_order.len()
    }

    /// Depth of the overlay stack.
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    /// Name of the currently visible page.
    pub fn current_page_name(&self) -> Option<&str> {
        if let Some(top) = self.stack.last() {
            Some(top.as_str())
        } else {
            self.page_order.get(self.current_index).map(String::as_str)
        }
    }
}

impl Default for PageManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::button_manager::ButtonAction;

    fn sample_page(name: &str, count: usize) -> ButtonPage {
        let buttons = (0..count)
            .map(|i| ButtonConfig {
                label: format!("{name}-{i}"),
                icon_path: None,
                action: ButtonAction::SimCommand(format!("CMD_{name}_{i}")),
            })
            .collect();
        ButtonPage::with_buttons(name, buttons)
    }

    // ── Basic page management ──────────────────────────────────────

    #[test]
    fn test_empty_manager() {
        let mgr = PageManager::new();
        assert_eq!(mgr.page_count(), 0);
        assert!(mgr.current_page().is_err());
    }

    #[test]
    fn test_add_and_get_page() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Main", 4));
        assert_eq!(mgr.page_count(), 1);
        let page = mgr.current_page().unwrap();
        assert_eq!(page.name, "Main");
        assert_eq!(page.buttons.len(), 4);
    }

    #[test]
    fn test_current_buttons() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("P1", 3));
        let buttons = mgr.current_buttons().unwrap();
        assert_eq!(buttons.len(), 3);
    }

    // ── Switch by name ─────────────────────────────────────────────

    #[test]
    fn test_switch_page() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Alpha", 2));
        mgr.add_page(sample_page("Beta", 3));

        mgr.switch_page("Beta").unwrap();
        assert_eq!(mgr.current_page().unwrap().name, "Beta");
        assert_eq!(mgr.current_index(), 1);
    }

    #[test]
    fn test_switch_page_not_found() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("A", 1));
        assert!(mgr.switch_page("Z").is_err());
    }

    // ── Next / prev navigation ─────────────────────────────────────

    #[test]
    fn test_next_page() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("A", 1));
        mgr.add_page(sample_page("B", 1));
        mgr.add_page(sample_page("C", 1));

        assert_eq!(mgr.current_page().unwrap().name, "A");
        mgr.next_page().unwrap();
        assert_eq!(mgr.current_page().unwrap().name, "B");
        mgr.next_page().unwrap();
        assert_eq!(mgr.current_page().unwrap().name, "C");
    }

    #[test]
    fn test_next_page_wraps() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("A", 1));
        mgr.add_page(sample_page("B", 1));

        mgr.next_page().unwrap(); // -> B
        let page = mgr.next_page().unwrap(); // -> A (wrap)
        assert_eq!(page.name, "A");
    }

    #[test]
    fn test_prev_page() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("A", 1));
        mgr.add_page(sample_page("B", 1));
        mgr.add_page(sample_page("C", 1));

        mgr.switch_page("C").unwrap();
        mgr.prev_page().unwrap();
        assert_eq!(mgr.current_page().unwrap().name, "B");
    }

    #[test]
    fn test_prev_page_wraps() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("A", 1));
        mgr.add_page(sample_page("B", 1));

        // At index 0, prev should wrap to last.
        let page = mgr.prev_page().unwrap();
        assert_eq!(page.name, "B");
    }

    #[test]
    fn test_next_on_empty_errors() {
        let mut mgr = PageManager::new();
        assert!(mgr.next_page().is_err());
    }

    #[test]
    fn test_prev_on_empty_errors() {
        let mut mgr = PageManager::new();
        assert!(mgr.prev_page().is_err());
    }

    // ── Overlay stack ──────────────────────────────────────────────

    #[test]
    fn test_push_page_overlay() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Main", 4));
        mgr.add_page(sample_page("Overlay", 2));

        assert_eq!(mgr.current_page().unwrap().name, "Main");

        mgr.push_page("Overlay").unwrap();
        assert_eq!(mgr.stack_depth(), 1);
        assert_eq!(mgr.current_page().unwrap().name, "Overlay");
    }

    #[test]
    fn test_pop_page_overlay() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Main", 4));
        mgr.add_page(sample_page("Sub", 2));

        mgr.push_page("Sub").unwrap();
        let popped = mgr.pop_page().unwrap();
        assert_eq!(popped, "Sub");
        assert_eq!(mgr.stack_depth(), 0);
        assert_eq!(mgr.current_page().unwrap().name, "Main");
    }

    #[test]
    fn test_pop_empty_stack_errors() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Main", 1));
        assert!(mgr.pop_page().is_err());
    }

    #[test]
    fn test_push_unknown_page_errors() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Main", 1));
        assert!(mgr.push_page("Nope").is_err());
    }

    #[test]
    fn test_nested_overlay_stack() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Main", 1));
        mgr.add_page(sample_page("Sub1", 1));
        mgr.add_page(sample_page("Sub2", 1));

        mgr.push_page("Sub1").unwrap();
        mgr.push_page("Sub2").unwrap();
        assert_eq!(mgr.stack_depth(), 2);
        assert_eq!(mgr.current_page().unwrap().name, "Sub2");

        mgr.pop_page().unwrap();
        assert_eq!(mgr.current_page().unwrap().name, "Sub1");

        mgr.pop_page().unwrap();
        assert_eq!(mgr.current_page().unwrap().name, "Main");
    }

    // ── Page name ──────────────────────────────────────────────────

    #[test]
    fn test_current_page_name() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("Root", 2));
        assert_eq!(mgr.current_page_name(), Some("Root"));

        mgr.add_page(sample_page("Over", 1));
        mgr.push_page("Over").unwrap();
        assert_eq!(mgr.current_page_name(), Some("Over"));
    }

    #[test]
    fn test_current_page_name_empty() {
        let mgr = PageManager::new();
        assert_eq!(mgr.current_page_name(), None);
    }

    // ── Duplicate add is idempotent on order ───────────────────────

    #[test]
    fn test_add_same_name_updates_content() {
        let mut mgr = PageManager::new();
        mgr.add_page(sample_page("P", 2));
        mgr.add_page(sample_page("P", 5));
        assert_eq!(mgr.page_count(), 1);
        assert_eq!(mgr.current_page().unwrap().buttons.len(), 5);
    }
}
