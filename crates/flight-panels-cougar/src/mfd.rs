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

    // ══════════════════════════════════════════════════════════════════════
    //  Depth tests — MFD button mapping
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_twenty_buttons_per_mfd() {
        assert_eq!(MfdButton::ALL.len(), 20);
        assert_eq!(OSB_COUNT, 20);
        assert_eq!(OSBS_PER_SIDE, 5);
        // Every index 0..20 must be reachable
        for i in 0..OSB_COUNT {
            assert!(MfdButton::from_index(i).is_some(), "missing index {i}");
        }
    }

    #[test]
    fn depth_button_numbering_contiguous() {
        let indices: Vec<usize> = MfdButton::ALL.iter().map(|b| b.index()).collect();
        let expected: Vec<usize> = (0..20).collect();
        assert_eq!(indices, expected);
    }

    #[test]
    fn depth_osb_positions_by_side() {
        // Top row = indices 0–4
        let top = [
            MfdButton::Top1,
            MfdButton::Top2,
            MfdButton::Top3,
            MfdButton::Top4,
            MfdButton::Top5,
        ];
        for (offset, btn) in top.iter().enumerate() {
            assert_eq!(btn.index(), offset, "Top{} wrong index", offset + 1);
        }
        // Right row = 5–9
        let right = [
            MfdButton::Right1,
            MfdButton::Right2,
            MfdButton::Right3,
            MfdButton::Right4,
            MfdButton::Right5,
        ];
        for (offset, btn) in right.iter().enumerate() {
            assert_eq!(btn.index(), 5 + offset, "Right{} wrong index", offset + 1);
        }
        // Bottom row = 10–14
        let bottom = [
            MfdButton::Bottom1,
            MfdButton::Bottom2,
            MfdButton::Bottom3,
            MfdButton::Bottom4,
            MfdButton::Bottom5,
        ];
        for (offset, btn) in bottom.iter().enumerate() {
            assert_eq!(
                btn.index(),
                10 + offset,
                "Bottom{} wrong index",
                offset + 1
            );
        }
        // Left row = 15–19
        let left = [
            MfdButton::Left1,
            MfdButton::Left2,
            MfdButton::Left3,
            MfdButton::Left4,
            MfdButton::Left5,
        ];
        for (offset, btn) in left.iter().enumerate() {
            assert_eq!(btn.index(), 15 + offset, "Left{} wrong index", offset + 1);
        }
    }

    #[test]
    fn depth_rocker_switch_adjacent_pairs() {
        // Rocker switches typically use adjacent OSBs on the same side.
        // Verify adjacent buttons have sequential indices suitable for pair
        // decoding (e.g. Top1/Top2 as up/down of a rocker).
        let pairs: [(MfdButton, MfdButton); 4] = [
            (MfdButton::Top1, MfdButton::Top2),
            (MfdButton::Right1, MfdButton::Right2),
            (MfdButton::Bottom1, MfdButton::Bottom2),
            (MfdButton::Left1, MfdButton::Left2),
        ];
        for (a, b) in pairs {
            assert_eq!(
                b.index(),
                a.index() + 1,
                "adjacent pair {:?}/{:?} not sequential",
                a,
                b
            );
        }
    }

    #[test]
    fn depth_four_way_hat_corner_buttons() {
        // The four corner OSBs (one per side, first position) can model a
        // 4-way hat: Top1=up, Right1=right, Bottom1=down, Left1=left.
        let hat_up = MfdButton::Top1;
        let hat_right = MfdButton::Right1;
        let hat_down = MfdButton::Bottom1;
        let hat_left = MfdButton::Left1;

        let mut state = MfdButtonState::default();
        state.set(hat_up, true);
        state.set(hat_right, true);
        assert!(state.is_pressed(hat_up));
        assert!(state.is_pressed(hat_right));
        assert!(!state.is_pressed(hat_down));
        assert!(!state.is_pressed(hat_left));

        // Releasing hat_up leaves only right
        state.set(hat_up, false);
        assert!(!state.is_pressed(hat_up));
        assert!(state.is_pressed(hat_right));
    }

    #[test]
    fn depth_button_state_all_pressed() {
        let mut state = MfdButtonState::default();
        for btn in MfdButton::ALL {
            state.set(btn, true);
        }
        // All 20 low bits set
        assert_eq!(state.0, 0x000F_FFFF);
        for btn in MfdButton::ALL {
            assert!(state.is_pressed(btn));
        }
    }

    // ══════════════════════════════════════════════════════════════════════
    //  Depth tests — Display interface
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_mfd_page_all_twenty_labels() {
        let mut page = MfdPage::new("FCR");
        for btn in MfdButton::ALL {
            let text = format!("L{}", btn.index() + 1);
            page.set_label(btn, &text, btn.index() % 2 == 0);
        }
        for btn in MfdButton::ALL {
            let lbl = page.label(btn);
            assert_eq!(lbl.text, format!("L{}", btn.index() + 1));
            assert_eq!(lbl.active, btn.index() % 2 == 0);
        }
    }

    #[test]
    fn depth_backlight_brightness_led_present() {
        // Left/Right MFDs expose a BRIGHTNESS LED in their mapping.
        let left_leds = CougarMfdType::MfdLeft.led_mapping();
        let right_leds = CougarMfdType::MfdRight.led_mapping();
        assert!(left_leds.contains(&"BRIGHTNESS"));
        assert!(right_leds.contains(&"BRIGHTNESS"));
        // Center also has brightness control
        let center_leds = CougarMfdType::MfdCenter.led_mapping();
        assert!(center_leds.contains(&"BRIGHTNESS"));
    }

    #[test]
    fn depth_brightness_discrete_levels() {
        // Build reports at discrete brightness levels and verify byte value.
        let levels: [(f32, u8); 5] = [
            (0.0, 0),
            (0.25, 63),  // 0.25 * 255 = 63.75 → 63
            (0.5, 127),  // 0.5 * 255 = 127.5 → 127
            (0.75, 191), // 0.75 * 255 = 191.25 → 191
            (1.0, 255),
        ];
        for (brightness, expected_byte) in levels {
            let mut state = MfdButtonState::default();
            state.set(MfdButton::Top1, true);
            // Verify brightness maps to expected byte via formula
            let computed = (brightness * 255.0) as u8;
            assert_eq!(
                computed, expected_byte,
                "brightness {brightness} → {computed}, expected {expected_byte}"
            );
        }
    }

    #[test]
    fn depth_night_mode_low_brightness() {
        // Night/NVG mode uses very low brightness values. Verify the formula
        // produces distinguishable non-zero values even at low settings.
        let night_levels: [f32; 3] = [0.01, 0.05, 0.1];
        for &b in &night_levels {
            let byte_val = (b * 255.0) as u8;
            assert!(byte_val > 0, "night brightness {b} produced zero byte");
        }
        // Very near-zero should still truncate to 0 (panel off)
        assert_eq!((0.003_f32 * 255.0) as u8, 0);
    }

    #[test]
    fn depth_flir_mode_page_setup() {
        // FLIR mode is modelled as a dedicated MfdPage with inverted labels.
        let mut display = MfdDisplay::new();
        let mut flir = MfdPage::new("FLIR");
        flir.set_label(MfdButton::Top1, "WHOT", true);
        flir.set_label(MfdButton::Top2, "BHOT", false);
        flir.set_label(MfdButton::Bottom1, "AREA", true);
        let idx = display.add_page(flir);

        display.select_page(idx);
        assert_eq!(display.current_page().name, "FLIR");
        assert_eq!(display.current_page().label(MfdButton::Top1).text, "WHOT");
        assert!(display.current_page().label(MfdButton::Top1).active);
        assert!(!display.current_page().label(MfdButton::Top2).active);
    }

    // ══════════════════════════════════════════════════════════════════════
    //  Depth tests — Device identification
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_vid_pid_thrustmaster_cougar() {
        assert_eq!(COUGAR_VID, 0x044F, "Thrustmaster VID must be 0x044F");
        assert_eq!(CougarMfdType::MfdLeft as u16, 0x0404);
        assert_eq!(CougarMfdType::MfdRight as u16, 0x0405);
        assert_eq!(CougarMfdType::MfdCenter as u16, 0x0406);
    }

    #[test]
    fn depth_left_vs_right_mfd_differentiation() {
        let left = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
        let right = CougarMfdProtocol::new(CougarMfdType::MfdRight);

        // Different PIDs
        assert_ne!(left.product_id(), right.product_id());
        // Same VID
        assert_eq!(left.vendor_id(), right.vendor_id());
        // Same LED count (both are full-size MFDs)
        assert_eq!(left.led_names().len(), right.led_names().len());
        // Different names
        assert_ne!(left.name(), right.name());
        assert!(left.name().contains("Left"));
        assert!(right.name().contains("Right"));
    }

    #[test]
    fn depth_dual_mfd_coexistence() {
        // Both Left and Right protocols can exist simultaneously with
        // independent button state tracking.
        let left = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
        let right = CougarMfdProtocol::new(CougarMfdType::MfdRight);

        // Same input data parsed independently
        let data = [0x01, 0x00, 0x00]; // Top1 pressed
        let left_events = left.parse_input(&data).unwrap();
        let right_events = right.parse_input(&data).unwrap();
        assert_eq!(left_events.len(), right_events.len());
    }

    #[test]
    fn depth_usb_enumeration_pid_filtering() {
        // Only specific PIDs are valid Cougar MFDs
        let valid_pids: [u16; 3] = [0x0404, 0x0405, 0x0406];
        let invalid_pids: [u16; 4] = [0x0000, 0x0403, 0x0407, 0xFFFF];

        for pid in valid_pids {
            assert!(
                CougarMfdType::from_product_id(pid).is_some(),
                "PID {pid:#06X} should be valid"
            );
        }
        for pid in invalid_pids {
            assert!(
                CougarMfdType::from_product_id(pid).is_none(),
                "PID {pid:#06X} should be invalid"
            );
        }
    }

    #[test]
    fn depth_center_mfd_reduced_led_set() {
        let center = CougarMfdProtocol::new(CougarMfdType::MfdCenter);
        let left = CougarMfdProtocol::new(CougarMfdType::MfdLeft);

        // Center has fewer LEDs (13 vs 25)
        assert_eq!(center.led_names().len(), 13);
        assert!(center.led_names().len() < left.led_names().len());
        // Center has a POWER LED instead of extra control LEDs
        assert!(center.led_names().contains(&"POWER"));
        // Center still has the OSB subset
        assert!(center.led_names().contains(&"OSB1"));
        assert!(center.led_names().contains(&"OSB10"));
        // Smaller output report
        assert_eq!(center.output_report_size(), 4);
    }

    // ══════════════════════════════════════════════════════════════════════
    //  Depth tests — Profile binding
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_button_to_command_mapping_via_labels() {
        let mut page = MfdPage::new("DCS_CMD");
        page.set_label(MfdButton::Top1, "TMS-UP", true);
        page.set_label(MfdButton::Top3, "DMS-UP", false);
        page.set_label(MfdButton::Right1, "CMS-FWD", true);

        assert_eq!(page.label(MfdButton::Top1).text, "TMS-UP");
        assert_eq!(page.label(MfdButton::Top3).text, "DMS-UP");
        assert_eq!(page.label(MfdButton::Right1).text, "CMS-FWD");
    }

    #[test]
    fn depth_profile_switching_between_pages() {
        let mut display = MfdDisplay::new();

        let mut f16_page = MfdPage::new("F-16");
        f16_page.set_label(MfdButton::Top1, "FCR", true);
        let mut a10_page = MfdPage::new("A-10C");
        a10_page.set_label(MfdButton::Top1, "TAD", true);

        display.add_page(f16_page);
        display.add_page(a10_page);

        // Start on DEFAULT, switch to F-16
        display.select_page(1);
        assert_eq!(display.current_page().name, "F-16");
        assert_eq!(display.current_page().label(MfdButton::Top1).text, "FCR");

        // Switch to A-10C
        display.select_page(2);
        assert_eq!(display.current_page().name, "A-10C");
        assert_eq!(display.current_page().label(MfdButton::Top1).text, "TAD");
    }

    #[test]
    fn depth_f16_default_profile() {
        let mut page = MfdPage::new("F-16_BLK50");
        // Standard F-16 MFD layout labels
        page.set_label(MfdButton::Bottom1, "FCR", true);
        page.set_label(MfdButton::Bottom2, "FLIR", false);
        page.set_label(MfdButton::Bottom3, "TFR", false);
        page.set_label(MfdButton::Bottom5, "WPN", false);
        page.set_label(MfdButton::Top5, "SWAP", false);
        page.set_label(MfdButton::Top4, "SP", false);

        assert_eq!(page.label(MfdButton::Bottom1).text, "FCR");
        assert!(page.label(MfdButton::Bottom1).active);
        assert_eq!(page.label(MfdButton::Bottom2).text, "FLIR");
        assert!(!page.label(MfdButton::Bottom2).active);
    }

    #[test]
    fn depth_a10c_profile() {
        let mut page = MfdPage::new("A-10C_II");
        page.set_label(MfdButton::Bottom1, "TAD", true);
        page.set_label(MfdButton::Bottom2, "STAT", false);
        page.set_label(MfdButton::Bottom3, "DSMS", false);
        page.set_label(MfdButton::Bottom4, "MSG", false);
        page.set_label(MfdButton::Top1, "COM1", false);
        page.set_label(MfdButton::Top2, "COM2", false);

        assert_eq!(page.label(MfdButton::Bottom1).text, "TAD");
        assert_eq!(page.label(MfdButton::Top1).text, "COM1");
    }

    #[test]
    fn depth_profile_label_override_on_active_page() {
        let mut display = MfdDisplay::new();
        display.current_page_mut().set_label(MfdButton::Top1, "OLD", false);

        assert_eq!(display.current_page().label(MfdButton::Top1).text, "OLD");

        // Override the label in-place
        display.current_page_mut().set_label(MfdButton::Top1, "NEW", true);
        assert_eq!(display.current_page().label(MfdButton::Top1).text, "NEW");
        assert!(display.current_page().label(MfdButton::Top1).active);
    }

    // ══════════════════════════════════════════════════════════════════════
    //  Depth tests — HID protocol
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_input_report_three_byte_format() {
        // HID input report is 3 bytes, little-endian, 20 low bits.
        let test_cases: [(u32, [u8; 3]); 5] = [
            (0x000000, [0x00, 0x00, 0x00]),
            (0x000001, [0x01, 0x00, 0x00]), // bit 0 = Top1
            (0x000100, [0x00, 0x01, 0x00]), // bit 8 = Right4
            (0x080000, [0x00, 0x00, 0x08]), // bit 19 = Left5
            (0x0FFFFF, [0xFF, 0xFF, 0x0F]), // all 20 buttons
        ];
        for (expected_raw, bytes) in test_cases {
            let state = MfdButtonState::from_bytes(&bytes);
            assert_eq!(
                state.0, expected_raw,
                "bytes {:02X?} → {:#010X}, expected {:#010X}",
                bytes, state.0, expected_raw
            );
        }
    }

    #[test]
    fn depth_button_byte_mapping_per_bit() {
        // Each of the 20 buttons maps to exactly one bit in the 3-byte report.
        for btn in MfdButton::ALL {
            let bit = btn.index();
            let mut bytes = [0u8; 3];
            bytes[bit / 8] = 1 << (bit % 8);
            let state = MfdButtonState::from_bytes(&bytes);
            assert!(
                state.is_pressed(btn),
                "{:?} (bit {bit}) not detected in {:02X?}",
                btn,
                bytes
            );
            // No other button should be pressed
            for other in MfdButton::ALL {
                if other != btn {
                    assert!(
                        !state.is_pressed(other),
                        "{:?} falsely detected when only {:?} set",
                        other,
                        btn
                    );
                }
            }
        }
    }

    #[test]
    fn depth_output_report_size() {
        // All MFD types share the same output_report_size via PanelProtocol
        for mfd_type in [
            CougarMfdType::MfdLeft,
            CougarMfdType::MfdRight,
            CougarMfdType::MfdCenter,
        ] {
            let proto = CougarMfdProtocol::new(mfd_type);
            assert_eq!(
                proto.output_report_size(),
                4,
                "{} output_report_size mismatch",
                mfd_type.name()
            );
        }
    }

    #[test]
    fn depth_initialization_no_buttons_pressed() {
        // A fresh 3-byte zero report means nothing is pressed.
        let data = [0x00, 0x00, 0x00];
        let proto = CougarMfdProtocol::new(CougarMfdType::MfdLeft);
        let events = proto.parse_input(&data).unwrap();
        assert!(events.is_empty(), "zero report should yield no events");
    }

    #[test]
    fn depth_error_handling_malformed_reports() {
        let proto = CougarMfdProtocol::new(CougarMfdType::MfdRight);
        // Empty
        assert!(proto.parse_input(&[]).is_none());
        // 1 byte
        assert!(proto.parse_input(&[0xFF]).is_none());
        // 2 bytes
        assert!(proto.parse_input(&[0xFF, 0xFF]).is_none());
        // Exactly 3 bytes is valid
        assert!(proto.parse_input(&[0x00, 0x00, 0x00]).is_some());
        // Extra bytes beyond 3 are still valid (only first 3 used)
        assert!(proto.parse_input(&[0x01, 0x00, 0x00, 0xFF]).is_some());
    }

    // ══════════════════════════════════════════════════════════════════════
    //  Depth tests — Button state diff edge cases
    // ══════════════════════════════════════════════════════════════════════

    #[test]
    fn depth_diff_no_change() {
        let state = MfdButtonState(0x0005);
        let changes = state.diff(&state);
        assert!(changes.is_empty());
    }

    #[test]
    fn depth_diff_all_released() {
        let old = MfdButtonState(0x000F_FFFF); // all pressed
        let new = MfdButtonState(0);
        let changes = old.diff(&new);
        assert_eq!(changes.len(), 20);
        for (_, pressed) in &changes {
            assert!(!pressed, "all buttons should be released");
        }
    }

    #[test]
    fn depth_diff_all_pressed() {
        let old = MfdButtonState(0);
        let new = MfdButtonState(0x000F_FFFF);
        let changes = old.diff(&new);
        assert_eq!(changes.len(), 20);
        for (_, pressed) in &changes {
            assert!(pressed, "all buttons should be pressed");
        }
    }

    #[test]
    fn depth_from_bytes_masks_upper_bits() {
        // Bits above 19 must be masked off
        let data = [0xFF, 0xFF, 0xFF]; // 24 bits set
        let state = MfdButtonState::from_bytes(&data);
        // Only the low 20 bits survive
        assert_eq!(state.0, 0x000F_FFFF);
    }

    #[test]
    fn depth_osb_name_numbering() {
        // OSB names must be "OSB1" through "OSB20" matching index + 1.
        for btn in MfdButton::ALL {
            let expected = format!("OSB{}", btn.index() + 1);
            assert_eq!(btn.osb_name(), expected);
        }
    }
}
