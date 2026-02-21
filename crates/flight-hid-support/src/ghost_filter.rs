// SPDX-License-Identifier: MIT OR Apache-2.0

//! Ghost input filtering for HOTAS devices.
//!
//! Some HOTAS hardware (particularly X55/X56 mini-sticks) is known to generate
//! spurious button presses. This module provides filtering to mitigate these issues.
//!
//! # Overview
//!
//! Ghost inputs are false button activations caused by electrical noise, mechanical
//! bounce, or firmware bugs in HOTAS hardware. This module implements a two-stage
//! filtering pipeline to detect and suppress these phantom inputs while preserving
//! legitimate user actions.
//!
//! # Ghost Input Types
//!
//! - **Bounce**: Rapid on/off transitions faster than humanly possible (typically <10ms)
//! - **Impossible states**: Multiple mutually exclusive buttons pressed simultaneously
//!   (e.g., HAT switch reporting both up and down)
//! - **Stuck buttons**: Button appears held when physically released (handled by
//!   external watchdog, not this module)
//!
//! # Algorithm
//!
//! The [`GhostInputFilter`] applies two filtering stages in sequence:
//!
//! 1. **Debouncing** ([`ButtonDebouncer`]): Each button is tracked independently with
//!    its own timing state. A button state change is only accepted after it has been
//!    stable for the configured threshold duration (default 20ms). This filters out
//!    electrical bounce and rapid toggling that cannot represent real user input.
//!
//! 2. **Impossible State Detection** ([`ImpossibleStateDetector`]): After debouncing,
//!    the button state is checked against a list of "impossible" bitmasks. If all bits
//!    in any mask are simultaneously set (e.g., both Up and Down on a HAT switch),
//!    the state is rejected and the last valid state is returned instead.
//!
//! # Performance Characteristics
//!
//! - **Time complexity**: O(MAX_TRACKED_BUTTONS + M) per filter call, where M is the
//!   number of impossible masks
//! - **Memory**: Fixed-size arrays, no heap allocation in hot path
//! - **Latency**: Adds debounce_threshold latency to button press detection
//!
//! # Usage Example
//!
//! ```rust
//! use flight_hid_support::ghost_filter::{GhostInputFilter, presets};
//!
//! // Create filter for X56 throttle
//! let config = presets::saitek_x56_throttle();
//! let mut filter = GhostInputFilter::with_config(config);
//!
//! // Filter raw button states from HID reports
//! let raw_buttons: u32 = 0b0101; // Simulated ghost: Up + Down pressed
//! let filtered = filter.filter(raw_buttons);
//!
//! // Check ghost detection rate for diagnostics
//! println!("Ghost rate: {:.2}%", filter.ghost_rate() * 100.0);
//! ```
//!
//! # Device Presets
//!
//! The [`presets`] module provides pre-configured filters for common HOTAS devices:
//!
//! | Preset | Debounce | Description |
//! |--------|----------|-------------|
//! | [`presets::x55_x56_ministick`] | 25ms | X55/X56 mini-stick filtering |
//! | [`presets::saitek_x55_throttle`] | 30ms | Full X55 throttle support |
//! | [`presets::saitek_x56_throttle`] | 25ms | Full X56 throttle support |
//! | [`presets::thrustmaster_warthog`] | 15ms | A-10C replica HOTAS |
//! | [`presets::thrustmaster_t16000m`] | 20ms | Entry-level HALL sensor stick |
//! | [`presets::tflight_hotas4`] | 30ms | Budget HOTAS for consoles |
//! | [`presets::vkb_gladiator`] | 10ms | Premium contactless sensors |
//! | [`presets::aggressive`] | 50ms | Maximum filtering for noisy hardware |

use std::time::{Duration, Instant};

/// Default debounce threshold for button inputs (20 milliseconds).
///
/// This value represents a balance between:
/// - Filtering mechanical switch bounce (typically 5-15ms)
/// - Maintaining responsive feel (human reaction time ~100-200ms)
/// - Handling electrical noise in USB HID polling
///
/// For specific devices, use preset configurations which may specify
/// different thresholds based on switch quality.
pub const DEFAULT_DEBOUNCE_MS: u64 = 20;

/// Maximum number of buttons tracked by the filter.
///
/// This is derived from [`u32::BITS`] (32) since button states are represented
/// as a `u32` bitmask. Each bit position represents one button, allowing
/// tracking of buttons 0-31.
///
/// This constant ensures the filter's internal arrays are correctly sized
/// to handle all possible button indices without bounds checking at runtime.
pub const MAX_TRACKED_BUTTONS: usize = u32::BITS as usize;

/// Ghost input filter combining debouncing and impossible state detection.
///
/// This is the primary entry point for ghost input filtering. It combines
/// two filtering stages:
///
/// 1. **Debouncing**: Filters rapid button state changes that occur faster
///    than physically possible, typically caused by mechanical switch bounce.
///
/// 2. **Impossible state detection**: Identifies and rejects button combinations
///    that cannot occur on real hardware (e.g., opposite HAT directions).
///
/// # Thread Safety
///
/// This type is `!Sync` due to mutable internal state. For multi-threaded use,
/// wrap in appropriate synchronization primitives.
///
/// # Example
///
/// ```rust
/// use flight_hid_support::ghost_filter::GhostInputFilter;
///
/// let mut filter = GhostInputFilter::new();
/// let raw_button_state: u32 = 0b0001;
///
/// // Process button states from HID reports
/// let filtered_state = filter.filter(raw_button_state);
///
/// // Monitor ghost detection rate
/// if filter.ghost_rate() > 0.1 {
///     eprintln!("Warning: High ghost rate detected");
/// }
/// ```
#[derive(Debug)]
pub struct GhostInputFilter {
    debouncer: ButtonDebouncer,
    impossible_detector: ImpossibleStateDetector,
    stats: GhostFilterStats,
}

impl GhostInputFilter {
    /// Creates a new ghost input filter with default settings.
    ///
    /// Uses [`DEFAULT_DEBOUNCE_MS`] (20ms) debounce threshold and no
    /// impossible state masks. For device-specific filtering, use
    /// [`with_config`](Self::with_config) with a preset from [`presets`].
    pub fn new() -> Self {
        Self::with_config(GhostFilterConfig::default())
    }

    /// Creates a ghost input filter with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Filter configuration specifying debounce threshold and
    ///   impossible state masks. Use presets from [`presets`] module for
    ///   common HOTAS devices.
    ///
    /// # Example
    ///
    /// ```rust
    /// use flight_hid_support::ghost_filter::{GhostInputFilter, presets};
    ///
    /// let filter = GhostInputFilter::with_config(presets::saitek_x56_throttle());
    /// ```
    pub fn with_config(config: GhostFilterConfig) -> Self {
        Self {
            debouncer: ButtonDebouncer::new(config.debounce_threshold),
            impossible_detector: ImpossibleStateDetector::new(config.impossible_masks.clone()),
            stats: GhostFilterStats::default(),
        }
    }

    /// Filters a raw button state bitmask, returning the filtered state.
    ///
    /// This applies both debouncing and impossible state detection in sequence.
    /// The filtering pipeline is:
    ///
    /// 1. Raw input -> Debouncer -> Debounced state
    /// 2. Debounced state -> Impossible detector -> Final filtered state
    ///
    /// # Arguments
    ///
    /// * `raw` - Raw button state bitmask from HID report. Each bit represents
    ///   one button (bit 0 = button 0, etc.)
    ///
    /// # Returns
    ///
    /// Filtered button state with ghost inputs removed. May differ from input
    /// if debouncing suppressed a change or an impossible state was detected.
    ///
    /// # Performance
    ///
    /// This method is designed for real-time use at 250Hz+ polling rates.
    /// Time complexity is O(32 + M) where M is the number of impossible masks.
    pub fn filter(&mut self, raw: u32) -> u32 {
        let debounced = self.debouncer.filter(raw);
        let filtered = self.impossible_detector.filter(debounced);

        // Track statistics
        if raw != filtered {
            self.stats.total_filtered += 1;
        }
        if raw != debounced {
            self.stats.debounce_filtered += 1;
        }
        if debounced != filtered {
            self.stats.impossible_filtered += 1;
        }
        self.stats.total_samples += 1;

        filtered
    }

    /// Returns the current ghost detection rate as a ratio (0.0 to 1.0).
    ///
    /// This represents the fraction of input samples that were modified by
    /// filtering. A rate above 0.1 (10%) may indicate hardware issues.
    ///
    /// # Returns
    ///
    /// - `0.0` if no samples have been processed or no ghosts detected
    /// - `1.0` if every sample was modified by filtering
    pub fn ghost_rate(&self) -> f64 {
        if self.stats.total_samples == 0 {
            0.0
        } else {
            self.stats.total_filtered as f64 / self.stats.total_samples as f64
        }
    }

    /// Returns detailed filter statistics.
    ///
    /// Use this for diagnostics and monitoring filter effectiveness.
    pub fn stats(&self) -> &GhostFilterStats {
        &self.stats
    }

    /// Resets filter state and statistics.
    ///
    /// Call this when:
    /// - Switching to a different device
    /// - Starting a new flight session
    /// - After reconfiguring the filter
    ///
    /// This clears:
    /// - All debounce timing state
    /// - Last valid state for impossible detection
    /// - All accumulated statistics
    pub fn reset(&mut self) {
        self.debouncer.reset();
        self.impossible_detector.reset();
        self.stats = GhostFilterStats::default();
    }
}

impl Default for GhostInputFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for ghost input filtering.
///
/// Use [`Default::default()`] for basic filtering, or select a device-specific
/// preset from the [`presets`] module for optimal results.
///
/// # Example
///
/// ```rust
/// use flight_hid_support::ghost_filter::GhostFilterConfig;
/// use std::time::Duration;
///
/// let config = GhostFilterConfig {
///     debounce_threshold: Duration::from_millis(30),
///     impossible_masks: vec![
///         0b0011, // Buttons 0 and 1 cannot be pressed together
///         0b1100, // Buttons 2 and 3 cannot be pressed together
///     ],
/// };
/// ```
#[derive(Debug, Clone)]
pub struct GhostFilterConfig {
    /// Minimum time a button state must be stable before it is accepted.
    ///
    /// Lower values provide faster response but less filtering.
    /// Higher values filter more aggressively but add latency.
    ///
    /// Typical range: 10-50ms depending on switch quality.
    pub debounce_threshold: Duration,

    /// Bitmasks of mutually exclusive button combinations.
    ///
    /// Each mask defines a set of buttons that cannot physically be pressed
    /// simultaneously. If all bits in a mask are set in the input state,
    /// it is considered an impossible (ghost) state.
    ///
    /// Each mask should have at least 2 bits set; single-bit masks would
    /// block individual buttons entirely.
    pub impossible_masks: Vec<u32>,
}

impl Default for GhostFilterConfig {
    fn default() -> Self {
        Self {
            debounce_threshold: Duration::from_millis(DEFAULT_DEBOUNCE_MS),
            impossible_masks: Vec::new(),
        }
    }
}

/// Statistics from ghost input filtering.
///
/// Use these statistics to monitor filter effectiveness and diagnose
/// hardware issues. High filter rates may indicate:
///
/// - Worn or dirty switches (high debounce rate)
/// - Electrical interference (high impossible state rate)
/// - Incorrect device preset (unexpected impossible states)
///
/// # Example
///
/// ```rust
/// # use flight_hid_support::ghost_filter::GhostInputFilter;
/// let filter = GhostInputFilter::new();
/// let stats = filter.stats();
///
/// if stats.total_samples > 0 {
///     let debounce_pct = stats.debounce_filtered as f64 / stats.total_samples as f64 * 100.0;
///     println!("Debounce filter rate: {:.2}%", debounce_pct);
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GhostFilterStats {
    /// Total number of samples processed by [`GhostInputFilter::filter`].
    pub total_samples: u64,
    /// Number of samples where the output differed from the input.
    pub total_filtered: u64,
    /// Number of samples modified specifically by the debounce stage.
    pub debounce_filtered: u64,
    /// Number of samples modified specifically by impossible state detection.
    pub impossible_filtered: u64,
}

/// Per-button debouncer using independent timing for each button.
///
/// This debouncer tracks each button independently, allowing buttons to
/// change at different times while still filtering bounce on each one.
/// This is more accurate than a global debounce timer which would delay
/// all buttons when any one bounces.
///
/// # Algorithm
///
/// For each button (0 to [`MAX_TRACKED_BUTTONS`]-1):
///
/// 1. When raw state changes, record the timestamp
/// 2. Only accept the new state after it has been stable for `threshold` duration
/// 3. Until stable, continue outputting the previous accepted state
///
/// This filters both press bounce (rapid on-off-on) and release bounce
/// (rapid off-on-off) while maintaining the last known good state.
///
/// # Memory Layout
///
/// Uses a fixed-size array of `Option<Instant>` to track per-button timing.
/// Total size is `32 * size_of::<Option<Instant>>()` plus state fields.
#[derive(Debug)]
pub struct ButtonDebouncer {
    /// Minimum stable duration before accepting state change.
    threshold: Duration,
    /// Raw state from previous filter() call.
    last_state: u32,
    /// Per-button timestamp of last state change.
    last_change: [Option<Instant>; MAX_TRACKED_BUTTONS],
    /// Currently accepted (debounced) output state.
    output_state: u32,
}

impl ButtonDebouncer {
    /// Creates a new debouncer with the specified threshold.
    ///
    /// # Arguments
    ///
    /// * `threshold` - Minimum duration a button state must be stable before
    ///   it is accepted. Typical values are 10-50ms.
    ///
    /// # Example
    ///
    /// ```rust
    /// use flight_hid_support::ghost_filter::ButtonDebouncer;
    /// use std::time::Duration;
    ///
    /// let debouncer = ButtonDebouncer::new(Duration::from_millis(20));
    /// ```
    pub fn new(threshold: Duration) -> Self {
        Self {
            threshold,
            last_state: 0,
            last_change: [None; MAX_TRACKED_BUTTONS],
            output_state: 0,
        }
    }

    /// Filters a raw button state, applying per-button debounce logic.
    ///
    /// # Arguments
    ///
    /// * `raw` - Raw button state bitmask. Each bit represents one button.
    ///
    /// # Returns
    ///
    /// Debounced button state. Changes from the previous output only occur
    /// after the raw state has been stable for the threshold duration.
    ///
    /// # Note
    ///
    /// This method should be called at a consistent polling rate (e.g., 250Hz)
    /// for accurate timing. Irregular polling may cause timing drift.
    pub fn filter(&mut self, raw: u32) -> u32 {
        let now = Instant::now();
        let changed = raw ^ self.last_state;

        for i in 0..MAX_TRACKED_BUTTONS {
            let mask = 1u32 << i;
            if changed & mask != 0 {
                // Button state changed
                self.last_change[i] = Some(now);
            }

            // Check if enough time has passed to accept the new state
            if let Some(change_time) = self.last_change[i]
                && now.duration_since(change_time) >= self.threshold
            {
                // Accept the new state
                if raw & mask != 0 {
                    self.output_state |= mask;
                } else {
                    self.output_state &= !mask;
                }
            }
        }

        self.last_state = raw;
        self.output_state
    }

    /// Resets all debouncer state.
    ///
    /// After reset:
    /// - All buttons are considered released (output state = 0)
    /// - All timing information is cleared
    /// - Next state change will start fresh debounce timing
    pub fn reset(&mut self) {
        self.last_state = 0;
        self.last_change = [None; MAX_TRACKED_BUTTONS];
        self.output_state = 0;
    }
}

/// Detector for impossible button state combinations.
///
/// This detector identifies button states that cannot physically occur on
/// real hardware. For example, a HAT switch cannot report both "up" and "down"
/// simultaneously - if both bits are set, it indicates a ghost input.
///
/// # How It Works
///
/// Each "impossible mask" defines a set of buttons that are mutually exclusive.
/// If ALL bits in any mask are simultaneously set in the input state, the
/// entire state is rejected and the last valid state is returned instead.
///
/// # Example
///
/// ```rust
/// use flight_hid_support::ghost_filter::ImpossibleStateDetector;
///
/// // HAT switch: bits 0,1 = up/down, bits 2,3 = left/right
/// let mut detector = ImpossibleStateDetector::new(vec![
///     0b0011, // Up + Down impossible
///     0b1100, // Left + Right impossible
/// ]);
///
/// // Valid: only "up" pressed
/// assert_eq!(detector.filter(0b0001), 0b0001);
///
/// // Invalid: both "up" and "down" pressed - returns last valid state
/// assert_eq!(detector.filter(0b0011), 0b0001);
/// ```
#[derive(Debug)]
pub struct ImpossibleStateDetector {
    /// Each mask represents buttons that cannot all be pressed simultaneously.
    impossible_masks: Vec<u32>,
    /// Last state that passed validation (returned on impossible input).
    last_valid_state: u32,
}

impl ImpossibleStateDetector {
    /// Creates a new detector with the specified impossible state masks.
    ///
    /// # Arguments
    ///
    /// * `impossible_masks` - Vector of bitmasks. Each mask defines buttons
    ///   that cannot physically be pressed together. Masks should have at
    ///   least 2 bits set (single-bit masks are ignored).
    ///
    /// # Example
    ///
    /// ```rust
    /// use flight_hid_support::ghost_filter::ImpossibleStateDetector;
    ///
    /// // Define HAT switch constraints
    /// let detector = ImpossibleStateDetector::new(vec![
    ///     0b0011, // Bits 0 and 1 mutually exclusive
    ///     0b1100, // Bits 2 and 3 mutually exclusive
    /// ]);
    /// ```
    pub fn new(impossible_masks: Vec<u32>) -> Self {
        Self {
            impossible_masks,
            last_valid_state: 0,
        }
    }

    /// Filters a button state, replacing impossible states with the last valid state.
    ///
    /// # Arguments
    ///
    /// * `state` - Button state bitmask to validate.
    ///
    /// # Returns
    ///
    /// - If the state is valid: returns `state` unchanged
    /// - If the state is impossible: returns the last valid state
    ///
    /// The "last valid state" is updated each time a valid state passes through.
    pub fn filter(&mut self, state: u32) -> u32 {
        if self.is_impossible(state) {
            // Return last known valid state
            self.last_valid_state
        } else {
            self.last_valid_state = state;
            state
        }
    }

    /// Checks if a button state is impossible according to the configured masks.
    ///
    /// A state is considered impossible if ALL bits in ANY mask are set.
    /// Single-bit masks are ignored (they would block individual buttons).
    ///
    /// # Arguments
    ///
    /// * `state` - Button state bitmask to check.
    ///
    /// # Returns
    ///
    /// `true` if the state matches any impossible mask, `false` otherwise.
    pub fn is_impossible(&self, state: u32) -> bool {
        for mask in &self.impossible_masks {
            // If all bits in the mask are set, this is an impossible state
            if state & mask == *mask && mask.count_ones() > 1 {
                return true;
            }
        }
        false
    }

    /// Resets the detector state.
    ///
    /// Sets the last valid state to 0 (all buttons released).
    pub fn reset(&mut self) {
        self.last_valid_state = 0;
    }
}

/// Pre-configured ghost filters for known devices.
pub mod presets {
    use super::*;

    /// Ghost filter configured for X55/X56 mini-stick issues.
    ///
    /// The mini-sticks on X55/X56 throttles are known to generate ghost inputs,
    /// particularly when multiple directions appear pressed simultaneously.
    pub fn x55_x56_ministick() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(25),
            // Mini-stick cannot physically press opposite directions
            impossible_masks: vec![
                0b0011, // Up + Down impossible
                0b1100, // Left + Right impossible
            ],
        }
    }

    /// Ghost filter with aggressive debouncing for noisy hardware.
    pub fn aggressive() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(50),
            impossible_masks: Vec::new(),
        }
    }

    /// Ghost filter configured for T.Flight HOTAS 4 HAT switch.
    ///
    /// The T.Flight HOTAS 4 HAT switch can occasionally report impossible
    /// opposite directions simultaneously. This preset filters those states.
    pub fn tflight_hotas4() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(30),
            // HAT switch cannot physically press opposite directions
            impossible_masks: vec![
                0b0101, // Up + Down impossible
                0b1010, // Left + Right impossible
            ],
        }
    }

    /// Ghost filter configured for Saitek X55 Rhino throttle.
    ///
    /// The X55 throttle has known issues with:
    /// - Mini-stick generating ghost diagonal inputs (same as X56)
    /// - Rotary encoder switches producing bounce on mode transitions
    /// - E button cluster (4-way) occasionally reporting impossible states
    ///
    /// Button layout reference (relevant bits):
    /// - Bits 0-3: Mini-stick directions (Up/Down/Left/Right)
    /// - Bits 4-5: Rotary 1 CW/CCW
    /// - Bits 6-7: Rotary 2 CW/CCW
    /// - Bits 8-11: E button cluster (4-way switch)
    ///
    /// Uses 30ms debounce to handle rotary encoder bounce while maintaining
    /// acceptable button response for flight controls.
    pub fn saitek_x55_throttle() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(30),
            impossible_masks: vec![
                // Mini-stick: opposite directions impossible
                0b0000_0000_0011, // Mini-stick Up + Down (bits 0-1)
                0b0000_0000_1100, // Mini-stick Left + Right (bits 2-3)
                // Rotary encoders: CW + CCW simultaneously impossible
                0b0000_0011_0000, // Rotary 1 CW + CCW (bits 4-5)
                0b0000_1100_0000, // Rotary 2 CW + CCW (bits 6-7)
                // E button cluster: opposite directions impossible
                0b0001_0000_0000 | 0b0100_0000_0000, // E Up + Down (bits 8, 10)
                0b0010_0000_0000 | 0b1000_0000_0000, // E Left + Right (bits 9, 11)
            ],
        }
    }

    /// Ghost filter configured for Saitek/Logitech X56 Rhino throttle.
    ///
    /// The X56 throttle improves on the X55 design but still exhibits:
    /// - Mini-stick ghost inputs (improved but not eliminated)
    /// - Mode switch state glitches during transitions
    /// - Throttle detent micro-switches occasionally double-triggering
    ///
    /// Button layout reference (relevant bits):
    /// - Bits 0-3: Mini-stick directions (Up/Down/Left/Right)
    /// - Bits 4-5: Rotary 1 CW/CCW
    /// - Bits 6-7: Rotary 2 CW/CCW
    /// - Bits 8-10: Mode switch states (M1/M2/M3 - mutually exclusive)
    /// - Bits 11-12: Throttle detent switches
    ///
    /// Uses slightly shorter 25ms debounce as X56 has better switch quality,
    /// plus specific impossible state masks for the mode switch.
    pub fn saitek_x56_throttle() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(25),
            impossible_masks: vec![
                // Mini-stick: opposite directions impossible
                0b0_0000_0000_0011, // Mini-stick Up + Down (bits 0-1)
                0b0_0000_0000_1100, // Mini-stick Left + Right (bits 2-3)
                // Rotary encoders: CW + CCW simultaneously impossible
                0b0_0000_0011_0000, // Rotary 1 CW + CCW (bits 4-5)
                0b0_0000_1100_0000, // Rotary 2 CW + CCW (bits 6-7)
                // Mode switch: only one mode can be active (M1/M2/M3 at bits 8-10)
                0b0_0011_0000_0000, // M1 + M2 impossible
                0b0_0101_0000_0000, // M1 + M3 impossible
                0b0_0110_0000_0000, // M2 + M3 impossible
                0b0_0111_0000_0000, // All three modes impossible
            ],
        }
    }

    /// Ghost filter configured for Thrustmaster HOTAS Warthog.
    ///
    /// The Warthog is a high-quality replica of the A-10C throttle and stick.
    /// Known issues include:
    /// - Slew sensor (mini-stick on throttle) generating edge case ghost inputs
    /// - Boat switch (3-position) occasionally reporting multiple positions
    /// - China hat (4-way) on stick can report impossible diagonals under rapid use
    ///
    /// Button layout reference (throttle, relevant bits):
    /// - Bits 0-3: Slew sensor directions
    /// - Bits 4-6: Boat switch positions (FWD/MID/AFT - mutually exclusive)
    /// - Bits 7-10: China hat directions (stick)
    /// - Bits 11-13: Autopilot switches (3-position, mutually exclusive)
    ///
    /// Uses conservative 15ms debounce - Warthog uses high-quality Cherry MX
    /// style switches that need minimal debouncing, but the slew sensor
    /// benefits from some filtering.
    pub fn thrustmaster_warthog() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(15),
            impossible_masks: vec![
                // Slew sensor: opposite directions impossible
                0b0000_0000_0011, // Slew Up + Down (bits 0-1)
                0b0000_0000_1100, // Slew Left + Right (bits 2-3)
                // Boat switch: only one position possible (bits 4-6)
                0b0000_0011_0000, // FWD + MID impossible
                0b0000_0101_0000, // FWD + AFT impossible
                0b0000_0110_0000, // MID + AFT impossible
                // China hat: opposite directions impossible (bits 7-10)
                0b0001_1000_0000, // China Up + Down (bits 7, 9)
                0b0110_0000_0000, // China Left + Right (bits 8, 10)
                // Autopilot path switch (bits 11-13): mutually exclusive positions
                0b0001_1000_0000_0000, // PATH ALT + HDG impossible
                0b0010_1000_0000_0000, // PATH ALT + ALT impossible
                0b0011_0000_0000_0000, // PATH HDG + ALT impossible
            ],
        }
    }

    /// Ghost filter configured for Thrustmaster T.16000M FCS.
    ///
    /// The T.16000M is an entry-level ambidextrous joystick with HALL sensors.
    /// Known issues include:
    /// - HAT switch occasionally reporting ghost diagonals
    /// - Trigger micro-switch bounce on rapid fire
    /// - Throttle slider (when used with TWCS) has noisy edge detection
    ///
    /// Button layout reference (relevant bits):
    /// - Bits 0-3: POV HAT directions (N/E/S/W)
    /// - Bit 4: Primary trigger
    /// - Bits 5-7: Thumb buttons cluster
    /// - Bits 8-11: Base buttons
    ///
    /// Uses 20ms debounce as a balance between the higher-quality HALL sensors
    /// (which need less filtering) and the micro-switches (which benefit from
    /// moderate debouncing).
    pub fn thrustmaster_t16000m() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(20),
            impossible_masks: vec![
                // POV HAT: opposite directions impossible
                0b0101, // HAT N + S impossible (bits 0, 2)
                0b1010, // HAT E + W impossible (bits 1, 3)
            ],
        }
    }

    /// Ghost filter configured for VKB Gladiator NXT / NXT EVO.
    ///
    /// The VKB Gladiator series uses high-precision contactless sensors and
    /// quality micro-switches. Ghost inputs are rare but can occur on:
    /// - Ministick (analog stick with push) under extreme deflection
    /// - Hat switches during very rapid directional changes
    /// - Encoder buttons when rotating and pressing simultaneously
    ///
    /// Button layout reference (relevant bits):
    /// - Bits 0-3: Primary HAT directions
    /// - Bits 4-7: Secondary HAT / POV directions
    /// - Bits 8-11: Ministick directions (when in digital mode)
    /// - Bits 12-13: Encoder A CW/CCW
    /// - Bits 14-15: Encoder B CW/CCW
    ///
    /// Uses minimal 10ms debounce as VKB switches are high quality and
    /// over-filtering would hurt the premium feel. The impossible state
    /// masks catch the rare electrical glitches.
    pub fn vkb_gladiator() -> GhostFilterConfig {
        GhostFilterConfig {
            debounce_threshold: Duration::from_millis(10),
            impossible_masks: vec![
                // Primary HAT: opposite directions impossible
                0b0000_0000_0101, // HAT1 N + S (bits 0, 2)
                0b0000_0000_1010, // HAT1 E + W (bits 1, 3)
                // Secondary HAT: opposite directions impossible
                0b0000_0101_0000, // HAT2 N + S (bits 4, 6)
                0b0000_1010_0000, // HAT2 E + W (bits 5, 7)
                // Ministick digital mode: opposite directions impossible
                0b0101_0000_0000, // Mini N + S (bits 8, 10)
                0b1010_0000_0000, // Mini E + W (bits 9, 11)
                // Encoders: CW + CCW simultaneously impossible
                0b0011_0000_0000_0000, // Encoder A CW + CCW (bits 12-13)
                0b1100_0000_0000_0000, // Encoder B CW + CCW (bits 14-15)
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debouncer_stable_state() {
        let mut debouncer = ButtonDebouncer::new(Duration::from_millis(10));

        // Initial state should pass through after threshold
        assert_eq!(debouncer.filter(0b0001), 0);
        std::thread::sleep(Duration::from_millis(15));
        assert_eq!(debouncer.filter(0b0001), 0b0001);
    }

    #[test]
    fn test_debouncer_rejects_bounce() {
        let mut debouncer = ButtonDebouncer::new(Duration::from_millis(50));

        // Rapid changes should be rejected
        debouncer.filter(0b0001);
        debouncer.filter(0b0000);
        debouncer.filter(0b0001);

        // Should still be 0 since not enough time passed
        assert_eq!(debouncer.filter(0b0001), 0);
    }

    #[test]
    fn test_impossible_state_detection() {
        let mut detector = ImpossibleStateDetector::new(vec![0b0011]); // bits 0 and 1 can't both be set

        // Valid states pass through
        assert_eq!(detector.filter(0b0001), 0b0001);
        assert_eq!(detector.filter(0b0010), 0b0010);
        assert_eq!(detector.filter(0b0100), 0b0100);

        // Impossible state returns last valid
        assert_eq!(detector.filter(0b0011), 0b0100); // Returns last valid (0b0100)
    }

    #[test]
    fn test_ghost_filter_stats() {
        let mut filter = GhostInputFilter::new();

        // Process some samples
        filter.filter(0);
        filter.filter(0);
        filter.filter(0);

        assert_eq!(filter.stats().total_samples, 3);
        assert_eq!(filter.ghost_rate(), 0.0);
    }

    #[test]
    fn test_preset_configs() {
        let config = presets::x55_x56_ministick();
        assert_eq!(config.debounce_threshold, Duration::from_millis(25));
        assert!(!config.impossible_masks.is_empty());
    }

    #[test]
    fn test_preset_saitek_x55_throttle() {
        let config = presets::saitek_x55_throttle();

        // X55 uses 30ms debounce for rotary encoder bounce
        assert_eq!(config.debounce_threshold, Duration::from_millis(30));

        // Should have masks for mini-stick, rotaries, and E cluster
        assert!(config.impossible_masks.len() >= 6);

        // Verify mini-stick opposite directions are marked impossible
        assert!(config.impossible_masks.contains(&0b0011)); // Up + Down
        assert!(config.impossible_masks.contains(&0b1100)); // Left + Right

        // Verify rotary CW+CCW are marked impossible
        assert!(config.impossible_masks.contains(&0b0011_0000)); // Rotary 1
        assert!(config.impossible_masks.contains(&0b1100_0000)); // Rotary 2
    }

    #[test]
    fn test_preset_saitek_x56_throttle() {
        let config = presets::saitek_x56_throttle();

        // X56 uses 25ms debounce (better switch quality than X55)
        assert_eq!(config.debounce_threshold, Duration::from_millis(25));

        // Should have masks for mini-stick, rotaries, and mode switch
        assert!(config.impossible_masks.len() >= 7);

        // Verify mode switch combinations are marked impossible
        assert!(config.impossible_masks.contains(&0b0_0011_0000_0000)); // M1 + M2
        assert!(config.impossible_masks.contains(&0b0_0101_0000_0000)); // M1 + M3
        assert!(config.impossible_masks.contains(&0b0_0110_0000_0000)); // M2 + M3
        assert!(config.impossible_masks.contains(&0b0_0111_0000_0000)); // All three
    }

    #[test]
    fn test_preset_thrustmaster_warthog() {
        let config = presets::thrustmaster_warthog();

        // Warthog uses conservative 15ms debounce (high-quality switches)
        assert_eq!(config.debounce_threshold, Duration::from_millis(15));

        // Should have masks for slew sensor, boat switch, china hat, autopilot
        assert!(config.impossible_masks.len() >= 9);

        // Verify slew sensor opposite directions are marked impossible
        assert!(config.impossible_masks.contains(&0b0011)); // Slew Up + Down
        assert!(config.impossible_masks.contains(&0b1100)); // Slew Left + Right

        // Verify boat switch mutually exclusive states
        assert!(config.impossible_masks.contains(&0b0011_0000)); // FWD + MID
        assert!(config.impossible_masks.contains(&0b0101_0000)); // FWD + AFT
        assert!(config.impossible_masks.contains(&0b0110_0000)); // MID + AFT
    }

    #[test]
    fn test_preset_thrustmaster_t16000m() {
        let config = presets::thrustmaster_t16000m();

        // T.16000M uses 20ms debounce (HALL sensors + micro-switches)
        assert_eq!(config.debounce_threshold, Duration::from_millis(20));

        // Should have masks for POV HAT
        assert_eq!(config.impossible_masks.len(), 2);

        // Verify HAT opposite directions are marked impossible
        assert!(config.impossible_masks.contains(&0b0101)); // N + S
        assert!(config.impossible_masks.contains(&0b1010)); // E + W
    }

    #[test]
    fn test_preset_vkb_gladiator() {
        let config = presets::vkb_gladiator();

        // VKB uses minimal 10ms debounce (premium switches)
        assert_eq!(config.debounce_threshold, Duration::from_millis(10));

        // Should have masks for two HATs, ministick, and two encoders
        assert!(config.impossible_masks.len() >= 8);

        // Verify primary HAT opposite directions
        assert!(config.impossible_masks.contains(&0b0101)); // HAT1 N + S
        assert!(config.impossible_masks.contains(&0b1010)); // HAT1 E + W

        // Verify secondary HAT opposite directions
        assert!(config.impossible_masks.contains(&0b0101_0000)); // HAT2 N + S
        assert!(config.impossible_masks.contains(&0b1010_0000)); // HAT2 E + W

        // Verify encoder CW+CCW impossible
        assert!(config.impossible_masks.contains(&0b0011_0000_0000_0000)); // Encoder A
        assert!(config.impossible_masks.contains(&0b1100_0000_0000_0000)); // Encoder B
    }

    #[test]
    fn test_preset_debounce_thresholds_in_valid_range() {
        // All presets should have debounce between 10-50ms (typical HOTAS range)
        let presets = [
            ("x55_x56_ministick", presets::x55_x56_ministick()),
            ("aggressive", presets::aggressive()),
            ("tflight_hotas4", presets::tflight_hotas4()),
            ("saitek_x55_throttle", presets::saitek_x55_throttle()),
            ("saitek_x56_throttle", presets::saitek_x56_throttle()),
            ("thrustmaster_warthog", presets::thrustmaster_warthog()),
            ("thrustmaster_t16000m", presets::thrustmaster_t16000m()),
            ("vkb_gladiator", presets::vkb_gladiator()),
        ];

        for (name, config) in presets {
            let ms = config.debounce_threshold.as_millis();
            assert!(
                ms >= 10 && ms <= 50,
                "Preset {} has debounce {}ms outside valid 10-50ms range",
                name,
                ms
            );
        }
    }

    #[test]
    fn test_preset_impossible_masks_have_multiple_bits() {
        // Each impossible mask should have at least 2 bits set
        // (single bit masks would always trigger)
        let presets = [
            ("x55_x56_ministick", presets::x55_x56_ministick()),
            ("tflight_hotas4", presets::tflight_hotas4()),
            ("saitek_x55_throttle", presets::saitek_x55_throttle()),
            ("saitek_x56_throttle", presets::saitek_x56_throttle()),
            ("thrustmaster_warthog", presets::thrustmaster_warthog()),
            ("thrustmaster_t16000m", presets::thrustmaster_t16000m()),
            ("vkb_gladiator", presets::vkb_gladiator()),
        ];

        for (name, config) in presets {
            for (i, mask) in config.impossible_masks.iter().enumerate() {
                assert!(
                    mask.count_ones() >= 2,
                    "Preset {} mask {} (0b{:b}) has fewer than 2 bits set",
                    name,
                    i,
                    mask
                );
            }
        }
    }

    #[test]
    fn test_presets_create_working_filters() {
        // Verify all presets can be used to create functional filters
        let presets = [
            presets::x55_x56_ministick(),
            presets::aggressive(),
            presets::tflight_hotas4(),
            presets::saitek_x55_throttle(),
            presets::saitek_x56_throttle(),
            presets::thrustmaster_warthog(),
            presets::thrustmaster_t16000m(),
            presets::vkb_gladiator(),
        ];

        for config in presets {
            let mut filter = GhostInputFilter::with_config(config);

            // Filter should accept valid states
            let result = filter.filter(0);
            assert_eq!(result, 0);

            // Stats should track the sample
            assert_eq!(filter.stats().total_samples, 1);
        }
    }
}
