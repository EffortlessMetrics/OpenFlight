// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! StreamDeck device discovery and management
//!
//! Supports all StreamDeck hardware models: Original, Mini, XL, Plus, Pedal, and Neo.
//! Provides device enumeration, brightness control, and LCD strip display for Plus models.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, info, warn};

/// All known StreamDeck hardware models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamDeckModel {
    /// Original StreamDeck (15 keys, 3×5 grid, 72×72 px icons)
    Original,
    /// StreamDeck Mini (6 keys, 2×3 grid, 80×80 px icons)
    Mini,
    /// StreamDeck XL (32 keys, 4×8 grid, 96×96 px icons)
    Xl,
    /// StreamDeck MK.2 (15 keys, 3×5 grid, 72×72 px icons, USB-C)
    Mk2,
    /// StreamDeck Plus (8 keys + 4 dials + LCD strip, 120×120 px icons)
    Plus,
    /// StreamDeck Pedal (3 pedals, no display)
    Pedal,
    /// StreamDeck Neo (8 keys + touch strip, 96×96 px icons)
    Neo,
}

impl StreamDeckModel {
    /// Number of physical buttons (keys) on this model.
    pub fn key_count(&self) -> u8 {
        match self {
            Self::Original | Self::Mk2 => 15,
            Self::Mini => 6,
            Self::Xl => 32,
            Self::Plus => 8,
            Self::Pedal => 3,
            Self::Neo => 8,
        }
    }

    /// Grid layout as (rows, columns). `None` for non-grid devices.
    pub fn grid_layout(&self) -> Option<(u8, u8)> {
        match self {
            Self::Original | Self::Mk2 => Some((3, 5)),
            Self::Mini => Some((2, 3)),
            Self::Xl => Some((4, 8)),
            Self::Plus => Some((2, 4)),
            Self::Pedal => None,
            Self::Neo => Some((2, 4)),
        }
    }

    /// Icon pixel size for each key face. `None` for the Pedal (no display).
    pub fn icon_size(&self) -> Option<u32> {
        match self {
            Self::Original | Self::Mk2 => Some(72),
            Self::Mini => Some(80),
            Self::Xl => Some(96),
            Self::Plus => Some(120),
            Self::Pedal => None,
            Self::Neo => Some(96),
        }
    }

    /// Whether the device has an LCD touch strip (Plus / Neo).
    pub fn has_lcd_strip(&self) -> bool {
        matches!(self, Self::Plus | Self::Neo)
    }

    /// Whether the device has rotary dials (Plus only).
    pub fn has_dials(&self) -> bool {
        matches!(self, Self::Plus)
    }

    /// Number of rotary dials.
    pub fn dial_count(&self) -> u8 {
        match self {
            Self::Plus => 4,
            _ => 0,
        }
    }

    /// USB Vendor ID for Elgato devices.
    pub fn vendor_id() -> u16 {
        0x0FD9
    }

    /// USB Product ID for this model.
    pub fn product_id(&self) -> u16 {
        match self {
            Self::Original => 0x0060,
            Self::Mini => 0x0063,
            Self::Xl => 0x006C,
            Self::Mk2 => 0x0080,
            Self::Plus => 0x0084,
            Self::Pedal => 0x0086,
            Self::Neo => 0x009A,
        }
    }

    /// Identify model from a USB product ID, assuming the Elgato vendor ID.
    pub fn from_product_id(pid: u16) -> Option<Self> {
        Self::all().iter().find(|m| m.product_id() == pid).copied()
    }

    /// All known models.
    pub fn all() -> &'static [StreamDeckModel] {
        &[
            Self::Original,
            Self::Mini,
            Self::Xl,
            Self::Mk2,
            Self::Plus,
            Self::Pedal,
            Self::Neo,
        ]
    }

    /// Display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Original => "Stream Deck",
            Self::Mini => "Stream Deck Mini",
            Self::Xl => "Stream Deck XL",
            Self::Mk2 => "Stream Deck MK.2",
            Self::Plus => "Stream Deck +",
            Self::Pedal => "Stream Deck Pedal",
            Self::Neo => "Stream Deck Neo",
        }
    }

    /// Whether this model has a display (keys with LCD).
    pub fn has_display(&self) -> bool {
        !matches!(self, Self::Pedal)
    }
}

/// Device discovery errors.
#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("Device not found: {0}")]
    NotFound(String),
    #[error("Device communication error: {0}")]
    Communication(String),
    #[error("Brightness value out of range (0–100): {0}")]
    BrightnessOutOfRange(u8),
    #[error("LCD strip not supported on {0}")]
    LcdNotSupported(String),
}

/// Brightness setting clamped to 0–100 %.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brightness(u8);

impl Brightness {
    /// Create a new brightness value. Returns error if > 100.
    pub fn new(percent: u8) -> Result<Self, DeviceError> {
        if percent > 100 {
            return Err(DeviceError::BrightnessOutOfRange(percent));
        }
        Ok(Self(percent))
    }

    /// Raw percentage value.
    pub fn percent(&self) -> u8 {
        self.0
    }
}

impl Default for Brightness {
    fn default() -> Self {
        Self(70)
    }
}

/// LCD strip pixel dimensions for models that support it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LcdStripInfo {
    pub width: u32,
    pub height: u32,
}

impl LcdStripInfo {
    /// Get LCD strip info for a model, if the model supports it.
    pub fn for_model(model: StreamDeckModel) -> Option<Self> {
        match model {
            StreamDeckModel::Plus => Some(Self {
                width: 800,
                height: 100,
            }),
            StreamDeckModel::Neo => Some(Self {
                width: 248,
                height: 58,
            }),
            _ => None,
        }
    }
}

/// A discovered StreamDeck device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub model: StreamDeckModel,
    pub serial: Option<String>,
    pub firmware_version: Option<String>,
    pub connected: bool,
}

/// Manages discovery and state of connected StreamDeck devices.
pub struct DeviceManager {
    devices: HashMap<String, DeviceInfo>,
    brightness: HashMap<String, Brightness>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            brightness: HashMap::new(),
        }
    }

    /// Enumerate connected StreamDeck devices.
    ///
    /// In a real deployment this would use HID enumeration; here we return
    /// whatever has been registered via [`Self::register_device`].
    pub fn discover(&self) -> Vec<&DeviceInfo> {
        let devices: Vec<&DeviceInfo> = self.devices.values().filter(|d| d.connected).collect();
        info!(
            "Discovered {} connected StreamDeck device(s)",
            devices.len()
        );
        devices
    }

    /// Register a device (used by HID hot-plug callbacks).
    pub fn register_device(&mut self, info: DeviceInfo) {
        info!("Registered {} ({})", info.model.display_name(), info.id);
        let id = info.id.clone();
        self.brightness.insert(id.clone(), Brightness::default());
        self.devices.insert(id, info);
    }

    /// Mark a device as disconnected.
    pub fn disconnect_device(&mut self, device_id: &str) {
        if let Some(dev) = self.devices.get_mut(device_id) {
            dev.connected = false;
            info!("Device disconnected: {}", device_id);
        } else {
            warn!("Disconnect requested for unknown device: {}", device_id);
        }
    }

    /// Look up a device by id.
    pub fn get_device(&self, device_id: &str) -> Option<&DeviceInfo> {
        self.devices.get(device_id)
    }

    /// Set brightness for a device.
    pub fn set_brightness(
        &mut self,
        device_id: &str,
        brightness: Brightness,
    ) -> Result<(), DeviceError> {
        if !self.devices.contains_key(device_id) {
            return Err(DeviceError::NotFound(device_id.to_string()));
        }
        debug!(
            "Setting brightness for {} to {}%",
            device_id,
            brightness.percent()
        );
        self.brightness.insert(device_id.to_string(), brightness);
        Ok(())
    }

    /// Get current brightness for a device.
    pub fn get_brightness(&self, device_id: &str) -> Option<Brightness> {
        self.brightness.get(device_id).copied()
    }

    /// Get LCD strip info for a device, if supported.
    pub fn get_lcd_strip_info(&self, device_id: &str) -> Result<LcdStripInfo, DeviceError> {
        let dev = self
            .devices
            .get(device_id)
            .ok_or_else(|| DeviceError::NotFound(device_id.to_string()))?;
        LcdStripInfo::for_model(dev.model)
            .ok_or_else(|| DeviceError::LcdNotSupported(dev.model.display_name().to_string()))
    }

    /// List all known device ids.
    pub fn device_ids(&self) -> Vec<&str> {
        self.devices.keys().map(String::as_str).collect()
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── LCD strip segment rendering ──────────────────────────────────────────────

/// A single segment of the LCD touch strip display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcdSegment {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub label: String,
    pub value: String,
    pub background_color: String,
    pub text_color: String,
}

/// Builds an LCD strip layout for StreamDeck Plus / Neo models.
pub struct LcdStripLayout {
    pub model: StreamDeckModel,
    pub segments: Vec<LcdSegment>,
}

impl LcdStripLayout {
    /// Create a new layout for the given model.
    pub fn new(model: StreamDeckModel) -> Result<Self, DeviceError> {
        if !model.has_lcd_strip() {
            return Err(DeviceError::LcdNotSupported(
                model.display_name().to_string(),
            ));
        }
        Ok(Self {
            model,
            segments: Vec::new(),
        })
    }

    /// Add a segment to the layout.
    pub fn add_segment(&mut self, segment: LcdSegment) {
        self.segments.push(segment);
    }

    /// Build a 4-segment layout matching the 4 dials of the StreamDeck Plus.
    pub fn four_dial_layout(labels: [&str; 4]) -> Result<Self, DeviceError> {
        let mut layout = Self::new(StreamDeckModel::Plus)?;
        let strip = LcdStripInfo::for_model(StreamDeckModel::Plus).unwrap();
        let seg_w = strip.width / 4;

        for (i, label) in labels.iter().enumerate() {
            layout.add_segment(LcdSegment {
                x: seg_w * i as u32,
                y: 0,
                width: seg_w,
                height: strip.height,
                label: (*label).to_string(),
                value: String::new(),
                background_color: "#1A1A2E".to_string(),
                text_color: "#00D4FF".to_string(),
            });
        }
        Ok(layout)
    }
}

// ── Key and dial event types ─────────────────────────────────────────────────

/// A key press or release event from a StreamDeck device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key_index: u8,
    pub pressed: bool,
}

impl KeyEvent {
    /// Validate that the key index is within bounds for the given model.
    pub fn is_valid_for(&self, model: StreamDeckModel) -> bool {
        self.key_index < model.key_count()
    }
}

/// A rotary dial event from a StreamDeck Plus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialEvent {
    /// Dial index (0–3 on Plus).
    pub dial_index: u8,
    pub action: DialAction,
}

/// The action that occurred on a dial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialAction {
    /// Relative rotation in detent ticks (positive = clockwise).
    Rotate(i8),
    /// Dial knob pressed down.
    Press,
    /// Dial knob released.
    Release,
}

impl DialEvent {
    /// Validate that the dial index is within bounds for the given model.
    pub fn is_valid_for(&self, model: StreamDeckModel) -> bool {
        self.dial_index < model.dial_count()
    }
}

/// A touch event on an LCD strip (Plus / Neo).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TouchEvent {
    pub x: u32,
    pub y: u32,
    pub touch_type: TouchType,
}

/// The type of touch interaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TouchType {
    Tap,
    LongPress,
    Swipe { end_x: u32, end_y: u32 },
}

impl TouchEvent {
    /// Validate that the touch coordinates are within bounds for the model's LCD strip.
    pub fn is_valid_for(&self, model: StreamDeckModel) -> bool {
        if let Some(info) = LcdStripInfo::for_model(model) {
            self.x < info.width && self.y < info.height
        } else {
            false
        }
    }
}

// ── Image size validation ────────────────────────────────────────────────────

/// Validate that pixel dimensions match a model's expected icon size.
pub fn validate_image_size(model: StreamDeckModel, width: u32, height: u32) -> bool {
    match model.icon_size() {
        Some(expected) => width == expected && height == expected,
        None => false, // Pedal has no display
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Model metadata ─────────────────────────────────────────────────

    #[test]
    fn test_all_models_present() {
        assert_eq!(StreamDeckModel::all().len(), 7);
    }

    #[test]
    fn test_key_counts() {
        assert_eq!(StreamDeckModel::Original.key_count(), 15);
        assert_eq!(StreamDeckModel::Mini.key_count(), 6);
        assert_eq!(StreamDeckModel::Xl.key_count(), 32);
        assert_eq!(StreamDeckModel::Mk2.key_count(), 15);
        assert_eq!(StreamDeckModel::Plus.key_count(), 8);
        assert_eq!(StreamDeckModel::Pedal.key_count(), 3);
        assert_eq!(StreamDeckModel::Neo.key_count(), 8);
    }

    #[test]
    fn test_grid_layouts() {
        assert_eq!(StreamDeckModel::Original.grid_layout(), Some((3, 5)));
        assert_eq!(StreamDeckModel::Mini.grid_layout(), Some((2, 3)));
        assert_eq!(StreamDeckModel::Xl.grid_layout(), Some((4, 8)));
        assert_eq!(StreamDeckModel::Mk2.grid_layout(), Some((3, 5)));
        assert_eq!(StreamDeckModel::Plus.grid_layout(), Some((2, 4)));
        assert_eq!(StreamDeckModel::Pedal.grid_layout(), None);
        assert_eq!(StreamDeckModel::Neo.grid_layout(), Some((2, 4)));
    }

    #[test]
    fn test_icon_sizes() {
        assert_eq!(StreamDeckModel::Original.icon_size(), Some(72));
        assert_eq!(StreamDeckModel::Mini.icon_size(), Some(80));
        assert_eq!(StreamDeckModel::Xl.icon_size(), Some(96));
        assert_eq!(StreamDeckModel::Mk2.icon_size(), Some(72));
        assert_eq!(StreamDeckModel::Plus.icon_size(), Some(120));
        assert_eq!(StreamDeckModel::Pedal.icon_size(), None);
        assert_eq!(StreamDeckModel::Neo.icon_size(), Some(96));
    }

    #[test]
    fn test_lcd_strip_capability() {
        assert!(StreamDeckModel::Plus.has_lcd_strip());
        assert!(StreamDeckModel::Neo.has_lcd_strip());
        assert!(!StreamDeckModel::Original.has_lcd_strip());
        assert!(!StreamDeckModel::Mk2.has_lcd_strip());
        assert!(!StreamDeckModel::Pedal.has_lcd_strip());
    }

    #[test]
    fn test_dial_count() {
        assert_eq!(StreamDeckModel::Plus.dial_count(), 4);
        assert_eq!(StreamDeckModel::Original.dial_count(), 0);
        assert_eq!(StreamDeckModel::Mk2.dial_count(), 0);
        assert!(StreamDeckModel::Plus.has_dials());
        assert!(!StreamDeckModel::Xl.has_dials());
        assert!(!StreamDeckModel::Mk2.has_dials());
    }

    #[test]
    fn test_product_ids_unique() {
        let ids: Vec<u16> = StreamDeckModel::all()
            .iter()
            .map(|m| m.product_id())
            .collect();
        let mut deduped = ids.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len(), "product IDs must be unique");
    }

    #[test]
    fn test_display_names() {
        for model in StreamDeckModel::all() {
            assert!(!model.display_name().is_empty());
        }
    }

    #[test]
    fn test_mk2_matches_original_layout() {
        let orig = StreamDeckModel::Original;
        let mk2 = StreamDeckModel::Mk2;
        assert_eq!(orig.key_count(), mk2.key_count());
        assert_eq!(orig.grid_layout(), mk2.grid_layout());
        assert_eq!(orig.icon_size(), mk2.icon_size());
        assert_ne!(orig.product_id(), mk2.product_id());
    }

    #[test]
    fn test_has_display() {
        for model in StreamDeckModel::all() {
            if matches!(model, StreamDeckModel::Pedal) {
                assert!(!model.has_display());
            } else {
                assert!(model.has_display(), "{:?} should have a display", model);
            }
        }
    }

    // ── from_product_id round-trip ─────────────────────────────────────

    #[test]
    fn test_from_product_id_round_trip() {
        for model in StreamDeckModel::all() {
            let pid = model.product_id();
            let resolved = StreamDeckModel::from_product_id(pid);
            assert_eq!(resolved, Some(*model), "PID 0x{:04X} -> {:?}", pid, model);
        }
    }

    #[test]
    fn test_from_product_id_unknown() {
        assert_eq!(StreamDeckModel::from_product_id(0x0000), None);
        assert_eq!(StreamDeckModel::from_product_id(0xFFFF), None);
    }

    #[test]
    fn test_known_pids() {
        assert_eq!(StreamDeckModel::from_product_id(0x0060), Some(StreamDeckModel::Original));
        assert_eq!(StreamDeckModel::from_product_id(0x0063), Some(StreamDeckModel::Mini));
        assert_eq!(StreamDeckModel::from_product_id(0x006C), Some(StreamDeckModel::Xl));
        assert_eq!(StreamDeckModel::from_product_id(0x0080), Some(StreamDeckModel::Mk2));
        assert_eq!(StreamDeckModel::from_product_id(0x0084), Some(StreamDeckModel::Plus));
        assert_eq!(StreamDeckModel::from_product_id(0x0086), Some(StreamDeckModel::Pedal));
        assert_eq!(StreamDeckModel::from_product_id(0x009A), Some(StreamDeckModel::Neo));
    }

    #[test]
    fn test_vendor_id_constant() {
        assert_eq!(StreamDeckModel::vendor_id(), 0x0FD9);
    }

    // ── Grid key count consistency ─────────────────────────────────────

    #[test]
    fn test_grid_key_count_consistency() {
        for model in StreamDeckModel::all() {
            if let Some((rows, cols)) = model.grid_layout() {
                assert_eq!(
                    rows as u16 * cols as u16,
                    model.key_count() as u16,
                    "{:?} grid {}x{} != key_count {}",
                    model, rows, cols, model.key_count()
                );
            }
        }
    }

    // ── Brightness ─────────────────────────────────────────────────────

    #[test]
    fn test_brightness_valid() {
        assert!(Brightness::new(0).is_ok());
        assert!(Brightness::new(100).is_ok());
        assert_eq!(Brightness::new(70).unwrap().percent(), 70);
    }

    #[test]
    fn test_brightness_out_of_range() {
        assert!(Brightness::new(101).is_err());
        assert!(Brightness::new(255).is_err());
    }

    #[test]
    fn test_brightness_default() {
        assert_eq!(Brightness::default().percent(), 70);
    }

    #[test]
    fn test_brightness_boundary_values() {
        assert_eq!(Brightness::new(0).unwrap().percent(), 0);
        assert_eq!(Brightness::new(1).unwrap().percent(), 1);
        assert_eq!(Brightness::new(100).unwrap().percent(), 100);
    }

    // ── LCD strip info ─────────────────────────────────────────────────

    #[test]
    fn test_lcd_strip_info_plus() {
        let info = LcdStripInfo::for_model(StreamDeckModel::Plus).unwrap();
        assert_eq!(info.width, 800);
        assert_eq!(info.height, 100);
    }

    #[test]
    fn test_lcd_strip_info_neo() {
        let info = LcdStripInfo::for_model(StreamDeckModel::Neo).unwrap();
        assert!(info.width > 0);
    }

    #[test]
    fn test_lcd_strip_info_unsupported() {
        assert!(LcdStripInfo::for_model(StreamDeckModel::Original).is_none());
        assert!(LcdStripInfo::for_model(StreamDeckModel::Mk2).is_none());
        assert!(LcdStripInfo::for_model(StreamDeckModel::Pedal).is_none());
    }

    // ── Device manager ─────────────────────────────────────────────────

    #[test]
    fn test_device_manager_empty() {
        let mgr = DeviceManager::new();
        assert!(mgr.discover().is_empty());
        assert!(mgr.device_ids().is_empty());
    }

    #[test]
    fn test_register_and_discover() {
        let mut mgr = DeviceManager::new();
        mgr.register_device(DeviceInfo {
            id: "dev-1".into(),
            model: StreamDeckModel::Original,
            serial: Some("ABC123".into()),
            firmware_version: None,
            connected: true,
        });

        assert_eq!(mgr.discover().len(), 1);
        assert!(mgr.get_device("dev-1").is_some());
    }

    #[test]
    fn test_register_mk2_device() {
        let mut mgr = DeviceManager::new();
        mgr.register_device(DeviceInfo {
            id: "mk2-1".into(),
            model: StreamDeckModel::Mk2,
            serial: Some("MK2ABC".into()),
            firmware_version: Some("1.2.3".into()),
            connected: true,
        });

        let dev = mgr.get_device("mk2-1").unwrap();
        assert_eq!(dev.model, StreamDeckModel::Mk2);
        assert_eq!(dev.serial.as_deref(), Some("MK2ABC"));
    }

    #[test]
    fn test_register_all_models() {
        let mut mgr = DeviceManager::new();
        for (i, model) in StreamDeckModel::all().iter().enumerate() {
            mgr.register_device(DeviceInfo {
                id: format!("dev-{}", i),
                model: *model,
                serial: None,
                firmware_version: None,
                connected: true,
            });
        }
        assert_eq!(mgr.discover().len(), StreamDeckModel::all().len());
    }

    #[test]
    fn test_disconnect_device() {
        let mut mgr = DeviceManager::new();
        mgr.register_device(DeviceInfo {
            id: "dev-2".into(),
            model: StreamDeckModel::Xl,
            serial: None,
            firmware_version: None,
            connected: true,
        });

        mgr.disconnect_device("dev-2");
        assert!(mgr.discover().is_empty()); // connected filter
        assert!(mgr.get_device("dev-2").is_some()); // still known
    }

    #[test]
    fn test_disconnect_unknown_device_is_noop() {
        let mut mgr = DeviceManager::new();
        mgr.disconnect_device("nonexistent"); // should not panic
    }

    #[test]
    fn test_brightness_control() {
        let mut mgr = DeviceManager::new();
        mgr.register_device(DeviceInfo {
            id: "dev-3".into(),
            model: StreamDeckModel::Mini,
            serial: None,
            firmware_version: None,
            connected: true,
        });

        let b = Brightness::new(50).unwrap();
        mgr.set_brightness("dev-3", b).unwrap();
        assert_eq!(mgr.get_brightness("dev-3").unwrap().percent(), 50);
    }

    #[test]
    fn test_brightness_unknown_device() {
        let mut mgr = DeviceManager::new();
        let b = Brightness::new(50).unwrap();
        assert!(mgr.set_brightness("nope", b).is_err());
    }

    #[test]
    fn test_default_brightness_on_register() {
        let mut mgr = DeviceManager::new();
        mgr.register_device(DeviceInfo {
            id: "dev-bright".into(),
            model: StreamDeckModel::Neo,
            serial: None,
            firmware_version: None,
            connected: true,
        });
        assert_eq!(mgr.get_brightness("dev-bright").unwrap().percent(), 70);
    }

    #[test]
    fn test_lcd_strip_via_manager() {
        let mut mgr = DeviceManager::new();
        mgr.register_device(DeviceInfo {
            id: "plus-1".into(),
            model: StreamDeckModel::Plus,
            serial: None,
            firmware_version: None,
            connected: true,
        });
        mgr.register_device(DeviceInfo {
            id: "orig-1".into(),
            model: StreamDeckModel::Original,
            serial: None,
            firmware_version: None,
            connected: true,
        });

        assert!(mgr.get_lcd_strip_info("plus-1").is_ok());
        assert!(mgr.get_lcd_strip_info("orig-1").is_err());
        assert!(mgr.get_lcd_strip_info("unknown").is_err());
    }

    // ── LCD strip layout ───────────────────────────────────────────────

    #[test]
    fn test_four_dial_layout() {
        let layout = LcdStripLayout::four_dial_layout(["HDG", "ALT", "SPD", "VS"]).unwrap();
        assert_eq!(layout.segments.len(), 4);
        assert_eq!(layout.segments[0].label, "HDG");
        // Each segment should be 200 px wide (800 / 4).
        assert_eq!(layout.segments[0].width, 200);
    }

    #[test]
    fn test_lcd_layout_unsupported_model() {
        assert!(LcdStripLayout::new(StreamDeckModel::Original).is_err());
        assert!(LcdStripLayout::new(StreamDeckModel::Mk2).is_err());
    }

    #[test]
    fn test_lcd_layout_neo_supported() {
        assert!(LcdStripLayout::new(StreamDeckModel::Neo).is_ok());
    }

    // ── Key events ─────────────────────────────────────────────────────

    #[test]
    fn test_key_event_valid_for_all_models() {
        for model in StreamDeckModel::all() {
            let last_valid = KeyEvent { key_index: model.key_count() - 1, pressed: true };
            assert!(last_valid.is_valid_for(*model), "{:?} key {} should be valid", model, last_valid.key_index);

            let out_of_bounds = KeyEvent { key_index: model.key_count(), pressed: false };
            assert!(!out_of_bounds.is_valid_for(*model), "{:?} key {} should be invalid", model, out_of_bounds.key_index);
        }
    }

    #[test]
    fn test_key_event_press_release() {
        let press = KeyEvent { key_index: 0, pressed: true };
        let release = KeyEvent { key_index: 0, pressed: false };
        assert_ne!(press, release);
        assert!(press.is_valid_for(StreamDeckModel::Mini));
        assert!(release.is_valid_for(StreamDeckModel::Mini));
    }

    #[test]
    fn test_key_event_original_15_keys() {
        for i in 0..15 {
            assert!(KeyEvent { key_index: i, pressed: true }.is_valid_for(StreamDeckModel::Original));
        }
        assert!(!KeyEvent { key_index: 15, pressed: true }.is_valid_for(StreamDeckModel::Original));
    }

    #[test]
    fn test_key_event_mini_6_keys() {
        for i in 0..6 {
            assert!(KeyEvent { key_index: i, pressed: true }.is_valid_for(StreamDeckModel::Mini));
        }
        assert!(!KeyEvent { key_index: 6, pressed: true }.is_valid_for(StreamDeckModel::Mini));
    }

    #[test]
    fn test_key_event_xl_32_keys() {
        for i in 0..32 {
            assert!(KeyEvent { key_index: i, pressed: true }.is_valid_for(StreamDeckModel::Xl));
        }
        assert!(!KeyEvent { key_index: 32, pressed: true }.is_valid_for(StreamDeckModel::Xl));
    }

    #[test]
    fn test_key_event_mk2_15_keys() {
        for i in 0..15 {
            assert!(KeyEvent { key_index: i, pressed: true }.is_valid_for(StreamDeckModel::Mk2));
        }
        assert!(!KeyEvent { key_index: 15, pressed: true }.is_valid_for(StreamDeckModel::Mk2));
    }

    #[test]
    fn test_key_event_pedal_3_keys() {
        for i in 0..3 {
            assert!(KeyEvent { key_index: i, pressed: true }.is_valid_for(StreamDeckModel::Pedal));
        }
        assert!(!KeyEvent { key_index: 3, pressed: true }.is_valid_for(StreamDeckModel::Pedal));
    }

    // ── Dial events ────────────────────────────────────────────────────

    #[test]
    fn test_dial_event_valid_for_plus() {
        for i in 0..4 {
            let evt = DialEvent { dial_index: i, action: DialAction::Press };
            assert!(evt.is_valid_for(StreamDeckModel::Plus));
        }
        let evt = DialEvent { dial_index: 4, action: DialAction::Press };
        assert!(!evt.is_valid_for(StreamDeckModel::Plus));
    }

    #[test]
    fn test_dial_event_invalid_for_non_plus() {
        for model in StreamDeckModel::all() {
            if *model != StreamDeckModel::Plus {
                let evt = DialEvent { dial_index: 0, action: DialAction::Rotate(1) };
                assert!(!evt.is_valid_for(*model), "{:?} should not accept dials", model);
            }
        }
    }

    #[test]
    fn test_dial_rotation_directions() {
        let cw = DialEvent { dial_index: 0, action: DialAction::Rotate(1) };
        let ccw = DialEvent { dial_index: 0, action: DialAction::Rotate(-1) };
        assert_ne!(cw, ccw);
        assert!(cw.is_valid_for(StreamDeckModel::Plus));
        assert!(ccw.is_valid_for(StreamDeckModel::Plus));
    }

    #[test]
    fn test_dial_press_release() {
        let press = DialEvent { dial_index: 2, action: DialAction::Press };
        let release = DialEvent { dial_index: 2, action: DialAction::Release };
        assert_ne!(press, release);
    }

    // ── Touch events ───────────────────────────────────────────────────

    #[test]
    fn test_touch_event_valid_for_plus() {
        let evt = TouchEvent { x: 400, y: 50, touch_type: TouchType::Tap };
        assert!(evt.is_valid_for(StreamDeckModel::Plus));
    }

    #[test]
    fn test_touch_event_out_of_bounds() {
        let evt = TouchEvent { x: 800, y: 50, touch_type: TouchType::Tap };
        assert!(!evt.is_valid_for(StreamDeckModel::Plus)); // x == width, out of bounds

        let evt2 = TouchEvent { x: 0, y: 100, touch_type: TouchType::Tap };
        assert!(!evt2.is_valid_for(StreamDeckModel::Plus)); // y == height
    }

    #[test]
    fn test_touch_event_invalid_for_no_lcd() {
        let evt = TouchEvent { x: 0, y: 0, touch_type: TouchType::Tap };
        assert!(!evt.is_valid_for(StreamDeckModel::Original));
        assert!(!evt.is_valid_for(StreamDeckModel::Pedal));
    }

    #[test]
    fn test_touch_swipe() {
        let evt = TouchEvent {
            x: 10, y: 10,
            touch_type: TouchType::Swipe { end_x: 790, end_y: 10 },
        };
        assert!(evt.is_valid_for(StreamDeckModel::Plus));
    }

    // ── Image size validation ──────────────────────────────────────────

    #[test]
    fn test_validate_image_size_all_models() {
        assert!(validate_image_size(StreamDeckModel::Original, 72, 72));
        assert!(validate_image_size(StreamDeckModel::Mk2, 72, 72));
        assert!(validate_image_size(StreamDeckModel::Mini, 80, 80));
        assert!(validate_image_size(StreamDeckModel::Xl, 96, 96));
        assert!(validate_image_size(StreamDeckModel::Plus, 120, 120));
        assert!(validate_image_size(StreamDeckModel::Neo, 96, 96));
    }

    #[test]
    fn test_validate_image_size_wrong_dimensions() {
        assert!(!validate_image_size(StreamDeckModel::Original, 96, 96));
        assert!(!validate_image_size(StreamDeckModel::Mini, 72, 72));
        assert!(!validate_image_size(StreamDeckModel::Xl, 72, 72));
        assert!(!validate_image_size(StreamDeckModel::Original, 72, 96));
    }

    #[test]
    fn test_validate_image_size_pedal_always_false() {
        assert!(!validate_image_size(StreamDeckModel::Pedal, 72, 72));
        assert!(!validate_image_size(StreamDeckModel::Pedal, 0, 0));
    }

    // ── Serialization round-trips ──────────────────────────────────────

    #[test]
    fn test_model_serialization_round_trip() {
        for model in StreamDeckModel::all() {
            let json = serde_json::to_string(model).unwrap();
            let parsed: StreamDeckModel = serde_json::from_str(&json).unwrap();
            assert_eq!(*model, parsed);
        }
    }

    #[test]
    fn test_device_info_serialization_round_trip() {
        let info = DeviceInfo {
            id: "test-1".into(),
            model: StreamDeckModel::Mk2,
            serial: Some("SER123".into()),
            firmware_version: Some("2.0.0".into()),
            connected: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: DeviceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, info.id);
        assert_eq!(parsed.model, info.model);
        assert_eq!(parsed.serial, info.serial);
    }

    #[test]
    fn test_key_event_serialization_round_trip() {
        let evt = KeyEvent { key_index: 7, pressed: true };
        let json = serde_json::to_string(&evt).unwrap();
        let parsed: KeyEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, parsed);
    }

    #[test]
    fn test_dial_event_serialization_round_trip() {
        let evt = DialEvent { dial_index: 1, action: DialAction::Rotate(-3) };
        let json = serde_json::to_string(&evt).unwrap();
        let parsed: DialEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, parsed);
    }

    #[test]
    fn test_touch_event_serialization_round_trip() {
        let evt = TouchEvent {
            x: 123, y: 45,
            touch_type: TouchType::Swipe { end_x: 456, end_y: 78 },
        };
        let json = serde_json::to_string(&evt).unwrap();
        let parsed: TouchEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, parsed);
    }
}
