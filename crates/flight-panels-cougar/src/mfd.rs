// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Cougar MFD data model — buttons, pages, and OSB labeling.
//!
//! Provides the pure data model for the Thrustmaster Cougar MFD's 20 OSB buttons
//! (5 per side × 4 sides), display page management, and per-button labeling.
//! The HID communication layer lives in [`super::cougar`].

use flight_panels_core::protocol::{PanelEvent, PanelProtocol};

use super::cougar::CougarMfdType;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Thrustmaster vendor ID.
pub const COUGAR_VID: u16 = 0x044F;

/// Number of OSB (Option Select Button) buttons per MFD.
pub const OSB_COUNT: usize = 20;

/// OSB buttons per side.
pub const OSBS_PER_SIDE: usize = 5;

// ─── MfdButton ────────────────────────────────────────────────────────────────

/// Identifies one of the 20 programmable OSB buttons arranged around the MFD
/// bezel (5 per side: top, right, bottom, left).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MfdButton {
    Top1,
    Top2,
    Top3,
    Top4,
    Top5,
    Right1,
    Right2,
    Right3,
    Right4,
    Right5,
    Bottom1,
    Bottom2,
    Bottom3,
    Bottom4,
    Bottom5,
    Left1,
    Left2,
    Left3,
    Left4,
    Left5,
}

impl MfdButton {
    /// All 20 buttons in OSB order (Top → Right → Bottom → Left).
    pub const ALL: [MfdButton; OSB_COUNT] = [
        Self::Top1,
        Self::Top2,
        Self::Top3,
        Self::Top4,
        Self::Top5,
        Self::Right1,
        Self::Right2,
        Self::Right3,
        Self::Right4,
        Self::Right5,
        Self::Bottom1,
        Self::Bottom2,
        Self::Bottom3,
        Self::Bottom4,
        Self::Bottom5,
        Self::Left1,
        Self::Left2,
        Self::Left3,
        Self::Left4,
        Self::Left5,
    ];

    /// 0-based index matching the HID report bit position / LED index.
    pub fn index(self) -> usize {
        match self {
            Self::Top1 => 0,
            Self::Top2 => 1,
            Self::Top3 => 2,
            Self::Top4 => 3,
            Self::Top5 => 4,
            Self::Right1 => 5,
            Self::Right2 => 6,
            Self::Right3 => 7,
            Self::Right4 => 8,
            Self::Right5 => 9,
            Self::Bottom1 => 10,
            Self::Bottom2 => 11,
            Self::Bottom3 => 12,
            Self::Bottom4 => 13,
            Self::Bottom5 => 14,
            Self::Left1 => 15,
            Self::Left2 => 16,
            Self::Left3 => 17,
            Self::Left4 => 18,
            Self::Left5 => 19,
        }
    }

    /// Construct from a 0-based index.
    pub fn from_index(idx: usize) -> Option<Self> {
        Self::ALL.get(idx).copied()
    }

    /// Short label used in diagnostics / log output (e.g. `"OSB1"` – `"OSB20"`).
    pub fn osb_name(self) -> &'static str {
        OSB_NAMES[self.index()]
    }
}

/// OSB name lookup table aligned with `MfdButton::index()`.
pub const OSB_NAMES: [&str; OSB_COUNT] = [
    "OSB1", "OSB2", "OSB3", "OSB4", "OSB5", "OSB6", "OSB7", "OSB8", "OSB9", "OSB10", "OSB11",
    "OSB12", "OSB13", "OSB14", "OSB15", "OSB16", "OSB17", "OSB18", "OSB19", "OSB20",
];

// ─── MfdButtonState ───────────────────────────────────────────────────────────

/// Bitfield tracking which of the 20 OSB buttons are currently pressed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MfdButtonState(pub u32);

impl MfdButtonState {
    /// Check whether a specific button is pressed.
    #[inline]
    pub fn is_pressed(&self, button: MfdButton) -> bool {
        (self.0 >> button.index()) & 1 == 1
    }

    /// Set or clear the bit for `button`.
    #[inline]
    pub fn set(&mut self, button: MfdButton, pressed: bool) {
        if pressed {
            self.0 |= 1 << button.index();
        } else {
            self.0 &= !(1 << button.index());
        }
    }

    /// Parse from the first 3 bytes of a HID report (little-endian, 20 low bits).
    pub fn from_bytes(data: &[u8]) -> Self {
        if data.len() < 3 {
            return Self(0);
        }
        let raw = u32::from(data[0]) | (u32::from(data[1]) << 8) | (u32::from(data[2]) << 16);
        Self(raw & 0x000F_FFFF) // mask to 20 bits
    }

    /// Return the set of buttons that changed between `self` and `new`.
    pub fn diff(&self, new: &Self) -> Vec<(MfdButton, bool)> {
        let changed = self.0 ^ new.0;
        let mut result = Vec::new();
        for btn in MfdButton::ALL {
            if (changed >> btn.index()) & 1 == 1 {
                result.push((btn, new.is_pressed(btn)));
            }
        }
        result
    }
}

// ─── OsbLabel ─────────────────────────────────────────────────────────────────

/// User-facing label for a single OSB button.
#[derive(Debug, Clone, Default)]
pub struct OsbLabel {
    /// Short text shown next to the button (e.g. "NAV", "TGP", "A-A").
    pub text: String,
    /// Whether the button is currently active / highlighted.
    pub active: bool,
}

// ─── MfdPage ──────────────────────────────────────────────────────────────────

/// One logical display page. Each page defines its own OSB labels.
#[derive(Debug, Clone)]
pub struct MfdPage {
    pub name: String,
    pub labels: [OsbLabel; OSB_COUNT],
}

impl MfdPage {
    /// Create a blank page with the given name.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            labels: std::array::from_fn(|_| OsbLabel::default()),
        }
    }

    /// Set the label for a specific button.
    pub fn set_label(&mut self, button: MfdButton, text: &str, active: bool) {
        let idx = button.index();
        self.labels[idx].text = text.to_string();
        self.labels[idx].active = active;
    }

    /// Get the label for a specific button.
    pub fn label(&self, button: MfdButton) -> &OsbLabel {
        &self.labels[button.index()]
    }
}

// ─── MfdDisplay ───────────────────────────────────────────────────────────────

/// Manages a collection of pages and the currently visible page.
#[derive(Debug, Clone)]
pub struct MfdDisplay {
    pages: Vec<MfdPage>,
    current: usize,
}

impl MfdDisplay {
    /// Create a display with one blank default page.
    pub fn new() -> Self {
        Self {
            pages: vec![MfdPage::new("DEFAULT")],
            current: 0,
        }
    }

    /// Add a page and return its index.
    pub fn add_page(&mut self, page: MfdPage) -> usize {
        self.pages.push(page);
        self.pages.len() - 1
    }

    /// Select a page by index. No-op if out of range.
    pub fn select_page(&mut self, index: usize) {
        if index < self.pages.len() {
            self.current = index;
        }
    }

    /// Advance to the next page (wraps around).
    pub fn next_page(&mut self) {
        if !self.pages.is_empty() {
            self.current = (self.current + 1) % self.pages.len();
        }
    }

    /// Move to the previous page (wraps around).
    pub fn prev_page(&mut self) {
        if !self.pages.is_empty() {
            self.current = (self.current + self.pages.len() - 1) % self.pages.len();
        }
    }

    /// Currently visible page (read-only).
    pub fn current_page(&self) -> &MfdPage {
        &self.pages[self.current]
    }

    /// Currently visible page (mutable).
    pub fn current_page_mut(&mut self) -> &mut MfdPage {
        &mut self.pages[self.current]
    }

    /// Current page index.
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Total number of pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

impl Default for MfdDisplay {
    fn default() -> Self {
        Self::new()
    }
}

// ─── PanelProtocol implementation ────────────────────────────────────────────

/// Protocol driver for a Cougar MFD panel of a given type.
pub struct CougarMfdProtocol {
    mfd_type: CougarMfdType,
}

impl CougarMfdProtocol {
    pub fn new(mfd_type: CougarMfdType) -> Self {
        Self { mfd_type }
    }
}

impl PanelProtocol for CougarMfdProtocol {
    fn name(&self) -> &str {
        self.mfd_type.name()
    }

    fn vendor_id(&self) -> u16 {
        COUGAR_VID
    }

    fn product_id(&self) -> u16 {
        self.mfd_type as u16
    }

    fn led_names(&self) -> &[&'static str] {
        self.mfd_type.led_mapping()
    }

    fn output_report_size(&self) -> usize {
        // Cougar MFD LED report is compact: 3 bytes button state + LED control
        4
    }

    fn parse_input(&self, data: &[u8]) -> Option<Vec<PanelEvent>> {
        if data.len() < 3 {
            return None;
        }
        let state = MfdButtonState::from_bytes(data);
        let mut events = Vec::new();
        for btn in MfdButton::ALL {
            if state.is_pressed(btn) {
                events.push(PanelEvent::ButtonPress {
                    name: OSB_NAMES[btn.index()],
                });
            }
        }
        Some(events)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── MfdButton ────────────────────────────────────────────────────────

    #[test]
    fn test_all_buttons_have_unique_indices() {
        let mut seen = [false; OSB_COUNT];
        for btn in MfdButton::ALL {
            assert!(!seen[btn.index()], "duplicate index {}", btn.index());
            seen[btn.index()] = true;
        }
    }

    #[test]
    fn test_button_from_index_roundtrip() {
        for (i, btn) in MfdButton::ALL.iter().enumerate() {
            assert_eq!(MfdButton::from_index(i), Some(*btn));
            assert_eq!(btn.index(), i);
        }
        assert_eq!(MfdButton::from_index(20), None);
    }

    #[test]
    fn test_osb_names_aligned() {
        for btn in MfdButton::ALL {
            let name = btn.osb_name();
            assert!(name.starts_with("OSB"), "unexpected name: {name}");
        }
    }

    // ── MfdButtonState ───────────────────────────────────────────────────

    #[test]
    fn test_button_state_default_empty() {
        let state = MfdButtonState::default();
        for btn in MfdButton::ALL {
            assert!(!state.is_pressed(btn));
        }
    }

    #[test]
    fn test_button_state_set_and_clear() {
        let mut state = MfdButtonState::default();
        state.set(MfdButton::Top1, true);
        state.set(MfdButton::Left5, true);
        assert!(state.is_pressed(MfdButton::Top1));
        assert!(state.is_pressed(MfdButton::Left5));
        assert!(!state.is_pressed(MfdButton::Right3));

        state.set(MfdButton::Top1, false);
        assert!(!state.is_pressed(MfdButton::Top1));
    }

    #[test]
    fn test_button_state_from_bytes() {
        // Set bit 0 (Top1) and bit 19 (Left5)
        let data = [0x01, 0x00, 0x08]; // bit 0 + bit 19 = 0x080001
        let state = MfdButtonState::from_bytes(&data);
        assert!(state.is_pressed(MfdButton::Top1));
        assert!(state.is_pressed(MfdButton::Left5));
        assert!(!state.is_pressed(MfdButton::Top2));
    }

    #[test]
    fn test_button_state_from_bytes_too_short() {
        let state = MfdButtonState::from_bytes(&[0xFF, 0xFF]);
        assert_eq!(state.0, 0);
    }

    #[test]
    fn test_button_state_diff() {
        let old = MfdButtonState(0b0000_0000_0000_0000_0000_0001); // Top1
        let new = MfdButtonState(0b0000_0000_0000_0000_0000_0011); // Top1 + Top2
        let changes = old.diff(&new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0], (MfdButton::Top2, true));
    }

    // ── OsbLabel / MfdPage ───────────────────────────────────────────────

    #[test]
    fn test_page_set_and_get_label() {
        let mut page = MfdPage::new("MAIN");
        page.set_label(MfdButton::Top1, "NAV", true);
        let label = page.label(MfdButton::Top1);
        assert_eq!(label.text, "NAV");
        assert!(label.active);

        let blank = page.label(MfdButton::Bottom5);
        assert_eq!(blank.text, "");
        assert!(!blank.active);
    }

    // ── MfdDisplay ───────────────────────────────────────────────────────

    #[test]
    fn test_display_default_has_one_page() {
        let display = MfdDisplay::new();
        assert_eq!(display.page_count(), 1);
        assert_eq!(display.current_index(), 0);
        assert_eq!(display.current_page().name, "DEFAULT");
    }

    #[test]
    fn test_display_add_and_navigate() {
        let mut display = MfdDisplay::new();
        display.add_page(MfdPage::new("TAD"));
        display.add_page(MfdPage::new("TGP"));
        assert_eq!(display.page_count(), 3);

        display.next_page();
        assert_eq!(display.current_index(), 1);
        assert_eq!(display.current_page().name, "TAD");

        display.next_page();
        assert_eq!(display.current_page().name, "TGP");

        display.next_page(); // wraps
        assert_eq!(display.current_page().name, "DEFAULT");
    }

    #[test]
    fn test_display_prev_page_wraps() {
        let mut display = MfdDisplay::new();
        display.add_page(MfdPage::new("P2"));
        display.prev_page(); // wraps to last
        assert_eq!(display.current_index(), 1);
    }

    #[test]
    fn test_display_select_page() {
        let mut display = MfdDisplay::new();
        display.add_page(MfdPage::new("P2"));
        display.select_page(1);
        assert_eq!(display.current_page().name, "P2");
        display.select_page(99); // out of range, no-op
        assert_eq!(display.current_page().name, "P2");
    }

    // ── CougarMfdProtocol ────────────────────────────────────────────────

    #[test]
    fn test_protocol_metadata() {
        let proto = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
        assert_eq!(proto.name(), "Cougar MFD Left");
        assert_eq!(proto.vendor_id(), COUGAR_VID);
        assert_eq!(proto.product_id(), 0x0404);
        assert_eq!(proto.led_names().len(), 25); // 20 OSB + 5 extras
    }

    #[test]
    fn test_protocol_parse_input() {
        let proto = CougarMfdProtocol::new(CougarMfdType::MfdRight);
        let data = [0x05, 0x00, 0x00]; // bit 0 + bit 2 = Top1 + Top3
        let events = proto.parse_input(&data).unwrap();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PanelEvent::ButtonPress { name: "OSB1" }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PanelEvent::ButtonPress { name: "OSB3" }))
        );
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_protocol_parse_short_input() {
        let proto = CougarMfdProtocol::new(CougarMfdType::MfdCenter);
        assert!(proto.parse_input(&[0x01, 0x02]).is_none());
    }
}
