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
