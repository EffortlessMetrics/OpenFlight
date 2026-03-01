// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Platform priority classes, timer discipline, and core affinity.
//!
//! All hot-path structures are **zero-allocation** per ADR-004: fixed-size
//! arrays, no `Vec`, no `String`, no `Box`.

use std::time::Duration;

// =============================================================================
// PriorityClass
// =============================================================================

/// Platform-independent priority classes that map to OS-specific scheduling
/// mechanisms (MMCSS task names on Windows, nice values on Linux).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PriorityClass {
    /// Lowest priority — background work only.
    Idle,
    /// Below default OS scheduling priority.
    BelowNormal,
    /// Default OS scheduling priority.
    #[default]
    Normal,
    /// Elevated above normal threads.
    AboveNormal,
    /// High priority — latency-sensitive but not hard RT.
    High,
    /// Real-time — lowest latency, highest scheduling priority.
    RealTime,
}

impl PriorityClass {
    /// Map to a Windows MMCSS task name.
    ///
    /// These task names correspond to registry entries under
    /// `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile\Tasks`.
    pub const fn to_mmcss_task_name(self) -> &'static str {
        match self {
            PriorityClass::Idle => "Background",
            PriorityClass::BelowNormal => "Low Latency",
            PriorityClass::Normal => "Window Manager",
            PriorityClass::AboveNormal => "Playback",
            PriorityClass::High => "Games",
            PriorityClass::RealTime => "Pro Audio",
        }
    }

    /// Map to a Linux `nice` value (-20 = highest priority, 19 = lowest).
    pub const fn to_nice_value(self) -> i8 {
        match self {
            PriorityClass::Idle => 19,
            PriorityClass::BelowNormal => 10,
            PriorityClass::Normal => 0,
            PriorityClass::AboveNormal => -5,
            PriorityClass::High => -10,
            PriorityClass::RealTime => -20,
        }
    }

    /// All variants in ascending priority order.
    pub const ALL: [PriorityClass; 6] = [
        PriorityClass::Idle,
        PriorityClass::BelowNormal,
        PriorityClass::Normal,
        PriorityClass::AboveNormal,
        PriorityClass::High,
        PriorityClass::RealTime,
    ];
}

// =============================================================================
// CoreAffinity
// =============================================================================

/// CPU core affinity mask for thread pinning.
///
/// Uses a 64-bit bitmask where bit *N* means "allow scheduling on core *N*".
/// A mask of `0` means "no preference" (OS default scheduling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreAffinity {
    mask: u64,
    fallback_to_any: bool,
}

impl CoreAffinity {
    /// No affinity preference — let the OS schedule freely.
    pub const fn any() -> Self {
        Self {
            mask: 0,
            fallback_to_any: true,
        }
    }

    /// Pin to a single core. Returns `any()` if `core >= 64`.
    pub const fn single(core: u8) -> Self {
        if core as u64 >= 64 {
            return Self::any();
        }
        Self {
            mask: 1u64 << core as u64,
            fallback_to_any: false,
        }
    }

    /// Pin to a set of cores specified by bitmask.
    pub const fn from_mask(mask: u64) -> Self {
        Self {
            mask,
            fallback_to_any: false,
        }
    }

    /// Pin to a set of cores with fallback to any core if none are available.
    pub const fn from_mask_with_fallback(mask: u64) -> Self {
        Self {
            mask,
            fallback_to_any: true,
        }
    }

    /// The raw bitmask. A value of `0` means no preference.
    pub const fn mask(&self) -> u64 {
        self.mask
    }

    /// Whether the scheduler should fall back to any core when preferred
    /// cores are unavailable.
    pub const fn fallback_to_any(&self) -> bool {
        self.fallback_to_any
    }

    /// Number of cores set in the mask.
    pub const fn core_count(&self) -> u32 {
        self.mask.count_ones()
    }

    /// Check whether a specific core is included. Returns `false` if `core >= 64`.
    pub const fn has_core(&self, core: u8) -> bool {
        if core as u64 >= 64 {
            return false;
        }
        (self.mask >> (core as u64)) & 1 == 1
    }

    /// Add a core to the mask. Returns `self` unchanged if `core >= 64`.
    pub const fn with_core(self, core: u8) -> Self {
        if core as u64 >= 64 {
            return self;
        }
        Self {
            mask: self.mask | (1u64 << (core as u64)),
            fallback_to_any: self.fallback_to_any,
        }
    }

    /// Whether this represents "no preference".
    pub const fn is_any(&self) -> bool {
        self.mask == 0
    }
}

impl Default for CoreAffinity {
    fn default() -> Self {
        Self::any()
    }
}

// =============================================================================
// TimerDiscipline — zero-allocation jitter tracking
// =============================================================================

/// Number of samples kept in the circular buffer for percentile computation.
const DISCIPLINE_RING_SIZE: usize = 512;

/// Timer discipline: manages tick-rate targets and jitter tracking.
///
/// All state is stack-allocated — **zero heap allocation** on the hot path.
/// The circular buffer holds the last [`DISCIPLINE_RING_SIZE`] tick durations
/// for exact percentile computation.
#[derive(Debug, Clone, Copy)]
pub struct TimerDiscipline {
    /// Desired tick period in nanoseconds.
    target_period_ns: u64,
    /// Circular buffer of recent tick durations (nanoseconds).
    ring: [u64; DISCIPLINE_RING_SIZE],
    /// Write index into `ring` (wraps via u16 → mod DISCIPLINE_RING_SIZE).
    ring_idx: u16,
    /// Total ticks recorded.
    count: u64,
    // Running jitter statistics
    min_jitter_ns: u64,
    max_jitter_ns: u64,
    sum_jitter_ns: u64,
    sum_sq_jitter_ns: u128,
}

impl TimerDiscipline {
    /// Create a new discipline targeting the given tick rate.
    /// A `tick_rate_hz` of 0 is clamped to 1.
    pub const fn new(tick_rate_hz: u32) -> Self {
        let hz = if tick_rate_hz == 0 { 1 } else { tick_rate_hz };
        Self {
            target_period_ns: 1_000_000_000 / hz as u64,
            ring: [0u64; DISCIPLINE_RING_SIZE],
            ring_idx: 0,
            count: 0,
            min_jitter_ns: u64::MAX,
            max_jitter_ns: 0,
            sum_jitter_ns: 0,
            sum_sq_jitter_ns: 0,
        }
    }

    /// Create targeting the default 250 Hz rate.
    pub const fn new_250hz() -> Self {
        Self::new(250)
    }

    /// The target period in nanoseconds.
    pub const fn target_period_ns(&self) -> u64 {
        self.target_period_ns
    }

    /// Record the actual duration of one tick (**hot path — zero allocation**).
    ///
    /// Stores the absolute jitter (deviation from target period) for
    /// percentile reporting.
    #[inline]
    pub fn record_tick(&mut self, actual_duration: Duration) {
        let ns = actual_duration.as_nanos() as u64;
        let jitter = ns.abs_diff(self.target_period_ns);
        self.count += 1;
        self.sum_jitter_ns += jitter;
        self.sum_sq_jitter_ns += jitter as u128 * jitter as u128;
        if jitter < self.min_jitter_ns {
            self.min_jitter_ns = jitter;
        }
        if jitter > self.max_jitter_ns {
            self.max_jitter_ns = jitter;
        }
        let idx = self.ring_idx as usize % DISCIPLINE_RING_SIZE;
        self.ring[idx] = jitter;
        self.ring_idx = self.ring_idx.wrapping_add(1);
    }

    /// Produce a snapshot report of current jitter statistics.
    ///
    /// Jitter is the absolute deviation of each tick duration from the
    /// target period. Sorts a **stack-local copy** of the ring for exact
    /// percentiles.
    pub fn tick_report(&self) -> TimerReport {
        if self.count == 0 {
            return TimerReport::EMPTY;
        }

        let len = (self.count as usize).min(DISCIPLINE_RING_SIZE);
        let mut buf = self.ring;
        buf[..len].sort_unstable();

        let p50 = buf[len / 2];
        let p95_idx = (len * 95) / 100;
        let p99_idx = (len * 99) / 100;

        let p95 = buf[p95_idx.min(len.saturating_sub(1))];
        let p99 = buf[p99_idx.min(len.saturating_sub(1))];

        let mean = self.sum_jitter_ns / self.count;

        TimerReport {
            target_period_ns: self.target_period_ns,
            sample_count: self.count,
            min_jitter_ns: self.min_jitter_ns,
            max_jitter_ns: self.max_jitter_ns,
            mean_jitter_ns: mean,
            p50_jitter_ns: p50,
            p95_jitter_ns: p95,
            p99_jitter_ns: p99,
        }
    }

    /// Total number of ticks recorded.
    pub const fn count(&self) -> u64 {
        self.count
    }

    /// Reset all statistics and the ring buffer.
    pub fn reset(&mut self) {
        let rate = 1_000_000_000u64 / self.target_period_ns;
        *self = Self::new(rate as u32);
    }
}

// =============================================================================
// TimerReport
// =============================================================================

/// Snapshot of timer discipline statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerReport {
    /// Target period in nanoseconds.
    pub target_period_ns: u64,
    /// Number of samples used to compute this report.
    pub sample_count: u64,
    /// Minimum observed jitter — absolute deviation from target period (ns).
    pub min_jitter_ns: u64,
    /// Maximum observed jitter — absolute deviation from target period (ns).
    pub max_jitter_ns: u64,
    /// Mean jitter — absolute deviation from target period (ns).
    pub mean_jitter_ns: u64,
    /// 50th percentile jitter (ns).
    pub p50_jitter_ns: u64,
    /// 95th percentile jitter (ns).
    pub p95_jitter_ns: u64,
    /// 99th percentile jitter (ns).
    pub p99_jitter_ns: u64,
}

impl TimerReport {
    /// An empty report (no samples recorded).
    pub const EMPTY: Self = Self {
        target_period_ns: 0,
        sample_count: 0,
        min_jitter_ns: 0,
        max_jitter_ns: 0,
        mean_jitter_ns: 0,
        p50_jitter_ns: 0,
        p95_jitter_ns: 0,
        p99_jitter_ns: 0,
    };
}

// =============================================================================
// RtSchedulerConfig
// =============================================================================

/// Comprehensive configuration for the real-time scheduler.
///
/// Validated on construction — invalid combinations are rejected.
#[derive(Debug, Clone)]
pub struct RtSchedulerConfig {
    priority_class: PriorityClass,
    tick_rate_hz: u32,
    max_allowed_jitter_ns: u64,
    core_affinity: CoreAffinity,
    busy_spin_us: u32,
    pll_gain: f64,
}

/// Errors from [`RtSchedulerConfig`] validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// Tick rate must be between 1 and 10 000 Hz.
    InvalidTickRate(u32),
    /// Max allowed jitter must be positive and ≤ half the tick period.
    InvalidMaxJitter { jitter_ns: u64, half_period_ns: u64 },
    /// PLL gain must be in (0.0, 1.0).
    InvalidPllGain(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::InvalidTickRate(hz) => {
                write!(f, "tick rate {hz} Hz out of range [1, 10000]")
            }
            ConfigError::InvalidMaxJitter {
                jitter_ns,
                half_period_ns,
            } => write!(
                f,
                "max jitter {jitter_ns} ns exceeds half-period {half_period_ns} ns"
            ),
            ConfigError::InvalidPllGain(msg) => write!(f, "invalid PLL gain: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl RtSchedulerConfig {
    /// Create a validated scheduler configuration.
    pub fn new(
        priority_class: PriorityClass,
        tick_rate_hz: u32,
        max_allowed_jitter_ns: u64,
        core_affinity: CoreAffinity,
        busy_spin_us: u32,
        pll_gain: f64,
    ) -> Result<Self, ConfigError> {
        if tick_rate_hz == 0 || tick_rate_hz > 10_000 {
            return Err(ConfigError::InvalidTickRate(tick_rate_hz));
        }
        let period_ns = 1_000_000_000u64 / tick_rate_hz as u64;
        let half_period = period_ns / 2;
        if max_allowed_jitter_ns == 0 || max_allowed_jitter_ns > half_period {
            return Err(ConfigError::InvalidMaxJitter {
                jitter_ns: max_allowed_jitter_ns,
                half_period_ns: half_period,
            });
        }
        if pll_gain <= 0.0 || pll_gain >= 1.0 || pll_gain.is_nan() {
            return Err(ConfigError::InvalidPllGain(format!("{pll_gain}")));
        }
        Ok(Self {
            priority_class,
            tick_rate_hz,
            max_allowed_jitter_ns,
            core_affinity,
            busy_spin_us,
            pll_gain,
        })
    }

    /// Sensible defaults: 250 Hz, RealTime class, 500 µs max jitter.
    pub fn default_rt() -> Self {
        Self {
            priority_class: PriorityClass::RealTime,
            tick_rate_hz: 250,
            max_allowed_jitter_ns: 500_000,
            core_affinity: CoreAffinity::any(),
            busy_spin_us: 65,
            pll_gain: 0.001,
        }
    }

    pub fn priority_class(&self) -> PriorityClass {
        self.priority_class
    }

    pub fn tick_rate_hz(&self) -> u32 {
        self.tick_rate_hz
    }

    pub fn max_allowed_jitter_ns(&self) -> u64 {
        self.max_allowed_jitter_ns
    }

    pub fn core_affinity(&self) -> CoreAffinity {
        self.core_affinity
    }

    pub fn busy_spin_us(&self) -> u32 {
        self.busy_spin_us
    }

    pub fn pll_gain(&self) -> f64 {
        self.pll_gain
    }

    /// The tick period derived from the tick rate.
    pub fn period_ns(&self) -> u64 {
        1_000_000_000u64 / self.tick_rate_hz as u64
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- PriorityClass MMCSS mapping ---

    #[test]
    fn priority_class_mmcss_names() {
        assert_eq!(PriorityClass::RealTime.to_mmcss_task_name(), "Pro Audio");
        assert_eq!(PriorityClass::High.to_mmcss_task_name(), "Games");
        assert_eq!(PriorityClass::AboveNormal.to_mmcss_task_name(), "Playback");
        assert_eq!(PriorityClass::Normal.to_mmcss_task_name(), "Window Manager");
        assert_eq!(
            PriorityClass::BelowNormal.to_mmcss_task_name(),
            "Low Latency"
        );
        assert_eq!(PriorityClass::Idle.to_mmcss_task_name(), "Background");
    }

    #[test]
    fn priority_class_mmcss_all_non_empty() {
        for &pc in &PriorityClass::ALL {
            assert!(
                !pc.to_mmcss_task_name().is_empty(),
                "{pc:?} has empty MMCSS name"
            );
        }
    }

    // --- PriorityClass nice mapping ---

    #[test]
    fn priority_class_nice_values() {
        assert_eq!(PriorityClass::RealTime.to_nice_value(), -20);
        assert_eq!(PriorityClass::High.to_nice_value(), -10);
        assert_eq!(PriorityClass::AboveNormal.to_nice_value(), -5);
        assert_eq!(PriorityClass::Normal.to_nice_value(), 0);
        assert_eq!(PriorityClass::BelowNormal.to_nice_value(), 10);
        assert_eq!(PriorityClass::Idle.to_nice_value(), 19);
    }

    #[test]
    fn priority_class_nice_values_are_monotonically_decreasing() {
        let classes = PriorityClass::ALL;
        for w in classes.windows(2) {
            assert!(
                w[0].to_nice_value() > w[1].to_nice_value(),
                "{:?} (nice={}) should have higher nice value than {:?} (nice={})",
                w[0],
                w[0].to_nice_value(),
                w[1],
                w[1].to_nice_value(),
            );
        }
    }

    #[test]
    fn priority_class_nice_range() {
        for &pc in &PriorityClass::ALL {
            let nice = pc.to_nice_value();
            assert!(
                (-20..=19).contains(&nice),
                "{pc:?} nice value {nice} out of range"
            );
        }
    }

    // --- PriorityClass traits ---

    #[test]
    fn priority_class_default_is_normal() {
        assert_eq!(PriorityClass::default(), PriorityClass::Normal);
    }

    #[test]
    fn priority_class_ordering() {
        assert!(PriorityClass::Idle < PriorityClass::Normal);
        assert!(PriorityClass::Normal < PriorityClass::RealTime);
        assert!(PriorityClass::High < PriorityClass::RealTime);
    }

    // --- TimerDiscipline recording and statistics ---

    #[test]
    fn timer_discipline_empty_report() {
        let td = TimerDiscipline::new(250);
        let report = td.tick_report();
        assert_eq!(report, TimerReport::EMPTY);
        assert_eq!(td.count(), 0);
    }

    #[test]
    fn timer_discipline_target_period() {
        let td = TimerDiscipline::new(250);
        assert_eq!(td.target_period_ns(), 4_000_000); // 4ms
    }

    #[test]
    fn timer_discipline_single_tick() {
        let mut td = TimerDiscipline::new(250);
        td.record_tick(Duration::from_micros(4000)); // exact 4ms → jitter = 0
        let report = td.tick_report();
        assert_eq!(report.sample_count, 1);
        assert_eq!(report.min_jitter_ns, 0);
        assert_eq!(report.max_jitter_ns, 0);
        assert_eq!(report.mean_jitter_ns, 0);
    }

    #[test]
    fn timer_discipline_known_statistics() {
        let mut td = TimerDiscipline::new(250);
        // Record ticks with known durations (target = 4000µs)
        // Jitters: 100µs, 50µs, 0, 50µs, 100µs
        for us in [3900, 3950, 4000, 4050, 4100] {
            td.record_tick(Duration::from_micros(us));
        }
        let report = td.tick_report();
        assert_eq!(report.sample_count, 5);
        assert_eq!(report.min_jitter_ns, 0);
        assert_eq!(report.max_jitter_ns, 100_000);
        assert_eq!(report.mean_jitter_ns, 60_000);
        assert_eq!(report.target_period_ns, 4_000_000);
    }

    #[test]
    fn timer_discipline_p99_with_outlier() {
        let mut td = TimerDiscipline::new(250);
        // 99 ticks at 4ms (jitter=0), 1 outlier at 8ms (jitter=4ms)
        for _ in 0..99 {
            td.record_tick(Duration::from_micros(4000));
        }
        td.record_tick(Duration::from_micros(8000));

        let report = td.tick_report();
        assert_eq!(report.sample_count, 100);
        // p99 index = (100*99)/100 = 99, which is the outlier jitter
        assert_eq!(report.p99_jitter_ns, 4_000_000);
        // p50 should be 0 (on-target ticks)
        assert_eq!(report.p50_jitter_ns, 0);
    }

    #[test]
    fn timer_discipline_circular_buffer_wraps() {
        let mut td = TimerDiscipline::new(250);
        // Fill with 1ms ticks (jitter = |1ms - 4ms| = 3ms)
        for _ in 0..DISCIPLINE_RING_SIZE {
            td.record_tick(Duration::from_micros(1000));
        }
        // Overwrite with 5ms ticks (jitter = |5ms - 4ms| = 1ms)
        for _ in 0..DISCIPLINE_RING_SIZE {
            td.record_tick(Duration::from_micros(5000));
        }
        let report = td.tick_report();
        // Ring should now contain only the 1ms jitter values
        assert_eq!(report.p99_jitter_ns, 1_000_000);
        assert_eq!(report.p50_jitter_ns, 1_000_000);
    }

    #[test]
    fn timer_discipline_reset() {
        let mut td = TimerDiscipline::new(250);
        for _ in 0..100 {
            td.record_tick(Duration::from_micros(4000));
        }
        assert_eq!(td.count(), 100);
        td.reset();
        assert_eq!(td.count(), 0);
        assert_eq!(td.tick_report(), TimerReport::EMPTY);
        // Target period preserved
        assert_eq!(td.target_period_ns(), 4_000_000);
    }

    // --- CoreAffinity ---

    #[test]
    fn core_affinity_any_is_default() {
        let ca = CoreAffinity::default();
        assert!(ca.is_any());
        assert_eq!(ca.mask(), 0);
        assert!(ca.fallback_to_any());
    }

    #[test]
    fn core_affinity_single_core() {
        let ca = CoreAffinity::single(3);
        assert!(!ca.is_any());
        assert_eq!(ca.core_count(), 1);
        assert!(ca.has_core(3));
        assert!(!ca.has_core(0));
        assert!(!ca.has_core(4));
    }

    #[test]
    fn core_affinity_from_mask() {
        let ca = CoreAffinity::from_mask(0b1010); // cores 1 and 3
        assert_eq!(ca.core_count(), 2);
        assert!(!ca.has_core(0));
        assert!(ca.has_core(1));
        assert!(!ca.has_core(2));
        assert!(ca.has_core(3));
        assert!(!ca.fallback_to_any());
    }

    #[test]
    fn core_affinity_with_fallback() {
        let ca = CoreAffinity::from_mask_with_fallback(0b0100);
        assert!(ca.has_core(2));
        assert!(ca.fallback_to_any());
    }

    #[test]
    fn core_affinity_with_core_builder() {
        let ca = CoreAffinity::any().with_core(0).with_core(2).with_core(4);
        assert_eq!(ca.core_count(), 3);
        assert!(ca.has_core(0));
        assert!(ca.has_core(2));
        assert!(ca.has_core(4));
        assert!(!ca.has_core(1));
    }

    // --- RtSchedulerConfig ---

    #[test]
    fn rt_config_default_is_valid() {
        let cfg = RtSchedulerConfig::default_rt();
        assert_eq!(cfg.priority_class(), PriorityClass::RealTime);
        assert_eq!(cfg.tick_rate_hz(), 250);
        assert_eq!(cfg.max_allowed_jitter_ns(), 500_000);
        assert_eq!(cfg.period_ns(), 4_000_000);
        assert!(cfg.core_affinity().is_any());
    }

    #[test]
    fn rt_config_valid_construction() {
        let cfg = RtSchedulerConfig::new(
            PriorityClass::High,
            500,
            500_000,
            CoreAffinity::single(0),
            50,
            0.002,
        );
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.tick_rate_hz(), 500);
        assert_eq!(cfg.busy_spin_us(), 50);
    }

    #[test]
    fn rt_config_rejects_zero_tick_rate() {
        let result = RtSchedulerConfig::new(
            PriorityClass::Normal,
            0,
            100_000,
            CoreAffinity::any(),
            65,
            0.001,
        );
        assert!(matches!(result, Err(ConfigError::InvalidTickRate(0))));
    }

    #[test]
    fn rt_config_rejects_excessive_tick_rate() {
        let result = RtSchedulerConfig::new(
            PriorityClass::Normal,
            20_000,
            100,
            CoreAffinity::any(),
            65,
            0.001,
        );
        assert!(matches!(result, Err(ConfigError::InvalidTickRate(20_000))));
    }

    #[test]
    fn rt_config_rejects_jitter_exceeding_half_period() {
        // 100 Hz → 10ms period → half = 5ms = 5_000_000 ns
        let result = RtSchedulerConfig::new(
            PriorityClass::Normal,
            100,
            6_000_000, // > 5ms
            CoreAffinity::any(),
            65,
            0.001,
        );
        assert!(matches!(result, Err(ConfigError::InvalidMaxJitter { .. })));
    }

    #[test]
    fn rt_config_rejects_invalid_pll_gain() {
        let result = RtSchedulerConfig::new(
            PriorityClass::Normal,
            250,
            500_000,
            CoreAffinity::any(),
            65,
            0.0, // invalid: must be > 0
        );
        assert!(matches!(result, Err(ConfigError::InvalidPllGain(_))));

        let result = RtSchedulerConfig::new(
            PriorityClass::Normal,
            250,
            500_000,
            CoreAffinity::any(),
            65,
            1.0, // invalid: must be < 1
        );
        assert!(matches!(result, Err(ConfigError::InvalidPllGain(_))));
    }

    #[test]
    fn config_error_display() {
        let e = ConfigError::InvalidTickRate(0);
        assert!(format!("{e}").contains("out of range"));

        let e = ConfigError::InvalidMaxJitter {
            jitter_ns: 6_000_000,
            half_period_ns: 5_000_000,
        };
        assert!(format!("{e}").contains("exceeds half-period"));

        let e = ConfigError::InvalidPllGain("0.0".into());
        assert!(format!("{e}").contains("PLL gain"));
    }

    // --- Edge-case tests (review feedback) ---

    #[test]
    fn test_core_affinity_out_of_range() {
        // core 64 should fall back to any()
        let ca = CoreAffinity::single(64);
        assert!(ca.is_any());
        assert!(ca.fallback_to_any());

        // core 255 should also be safe
        let ca = CoreAffinity::single(255);
        assert!(ca.is_any());

        // has_core with out-of-range core returns false
        let ca = CoreAffinity::from_mask(0xFF);
        assert!(!ca.has_core(64));
        assert!(!ca.has_core(128));
        assert!(!ca.has_core(255));

        // with_core with out-of-range core is a no-op
        let ca = CoreAffinity::single(0);
        let ca2 = ca.with_core(64);
        assert_eq!(ca.mask(), ca2.mask());
        let ca3 = ca.with_core(255);
        assert_eq!(ca.mask(), ca3.mask());
    }

    #[test]
    fn test_timer_zero_tick_rate() {
        // Must not panic — clamped to 1 Hz
        let td = TimerDiscipline::new(0);
        assert_eq!(td.target_period_ns(), 1_000_000_000); // 1 second
        assert_eq!(td.count(), 0);
    }

    #[test]
    fn test_timer_high_tick_rate() {
        // Must not panic at high rates (period may round to 0 via integer division)
        let td = TimerDiscipline::new(u32::MAX);
        let _ = td.target_period_ns();
        assert_eq!(td.count(), 0);
    }
}
