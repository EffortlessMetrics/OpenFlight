// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Time helpers for monotonic and unit-safe conversions.

use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Timebase {
    start: Instant,
    unix_base_ns: u64,
}

/// Convert milliseconds to nanoseconds.
pub fn to_ns_from_ms(ms: u64) -> u64 {
    ms.saturating_mul(1_000_000)
}

/// Convert a duration to nanoseconds, saturating on overflow.
pub fn elapsed_to_ns(duration: Duration) -> u64 {
    let nanos = duration.as_nanos();
    if nanos > u128::from(u64::MAX) {
        u64::MAX
    } else {
        nanos as u64
    }
}

fn system_time_to_ns(time: SystemTime) -> u64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => elapsed_to_ns(duration),
        Err(_) => 0,
    }
}

fn timebase() -> &'static Timebase {
    static TIMEBASE: OnceLock<Timebase> = OnceLock::new();
    TIMEBASE.get_or_init(|| {
        let start = Instant::now();
        let unix_base_ns = system_time_to_ns(SystemTime::now());
        Timebase {
            start,
            unix_base_ns,
        }
    })
}

/// Monotonic time in nanoseconds since process start.
pub fn monotonic_now_ns() -> u64 {
    elapsed_to_ns(timebase().start.elapsed())
}

/// Unix epoch time (nanoseconds) captured at process start.
pub fn unix_base_ns() -> u64 {
    timebase().unix_base_ns
}

/// Unix epoch time in nanoseconds using monotonic time for delta.
pub fn unix_now_ns() -> u64 {
    unix_base_ns().saturating_add(monotonic_now_ns())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ns_from_ms_basic() {
        assert_eq!(to_ns_from_ms(0), 0);
        assert_eq!(to_ns_from_ms(1), 1_000_000);
        assert_eq!(to_ns_from_ms(1000), 1_000_000_000);
    }

    #[test]
    fn to_ns_from_ms_saturates_on_overflow() {
        // Very large value should saturate rather than overflow
        let result = to_ns_from_ms(u64::MAX);
        assert_eq!(result, u64::MAX, "should saturate on overflow");
    }

    #[test]
    fn elapsed_to_ns_zero_duration() {
        assert_eq!(elapsed_to_ns(Duration::ZERO), 0);
    }

    #[test]
    fn elapsed_to_ns_one_second() {
        assert_eq!(elapsed_to_ns(Duration::from_secs(1)), 1_000_000_000);
    }

    #[test]
    fn elapsed_to_ns_saturates_on_overflow() {
        // Duration::MAX is ~584 years; far exceeds u64 nanos
        let huge = Duration::MAX;
        let result = elapsed_to_ns(huge);
        assert_eq!(result, u64::MAX, "should saturate for huge durations");
    }

    #[test]
    fn monotonic_now_ns_is_non_negative() {
        let t = monotonic_now_ns();
        // Just check it doesn't panic and returns a reasonable value
        assert!(t < u64::MAX, "should not saturate immediately");
    }

    #[test]
    fn unix_now_ns_is_after_epoch() {
        let ns = unix_now_ns();
        // Should be after year 2020 (in ns since epoch): 2020-01-01 ≈ 1_577_836_800 * 1e9
        assert!(ns > 1_577_836_800_000_000_000, "should be after year 2020");
    }

    #[test]
    fn monotonic_now_ns_increases_over_time() {
        let t1 = monotonic_now_ns();
        // Sleep briefly to allow time to pass
        std::thread::sleep(Duration::from_millis(1));
        let t2 = monotonic_now_ns();
        assert!(t2 >= t1, "monotonic time should not go backwards");
    }
}
