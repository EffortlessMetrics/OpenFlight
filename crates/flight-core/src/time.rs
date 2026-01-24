// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Time helpers for monotonic and unit-safe conversions.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

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

/// Monotonic time in nanoseconds since process start.
pub fn monotonic_now_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    elapsed_to_ns(start.elapsed())
}
