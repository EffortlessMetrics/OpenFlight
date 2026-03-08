// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted tests to improve mutation kill rate in flight-core.
// Each test is designed to catch specific common mutation patterns:
// boundary (< vs <=), arithmetic (+ vs -), boolean (true vs false), return value.

use flight_core::calibration_store::AxisCalibration;
use flight_core::circuit_breaker::{
    CallResult, CircuitBreaker, CircuitBreakerConfig, CircuitState,
};
use flight_core::error_catalog::{ErrorCatalog, ErrorCategory};
use std::time::Duration;

// ── Circuit Breaker: boundary & threshold mutations ──────────────────────

#[test]
fn cb_exact_failure_threshold_opens_circuit() {
    // Catches >= vs > mutation on `failure_count >= failure_threshold`
    let cfg = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(cfg);

    // 2 failures: still closed (< threshold)
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed, "2 failures < 3 threshold");

    // Exactly 3 failures: must be open
    cb.record_failure();
    assert_eq!(
        cb.state(),
        CircuitState::Open,
        "exactly at threshold must open"
    );
}

#[test]
fn cb_exact_success_threshold_closes_from_half_open() {
    // Catches >= vs > on `success_count >= success_threshold`
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 2,
        timeout: Duration::from_millis(10),
    };
    let mut cb = CircuitBreaker::new(cfg);
    cb.record_failure();
    std::thread::sleep(Duration::from_millis(20));
    let _ = cb.call_allowed(); // → HalfOpen

    // 1 success: still half-open (< threshold)
    cb.record_success();
    assert_eq!(
        cb.state(),
        CircuitState::HalfOpen,
        "1 success < 2 threshold"
    );

    // Exactly 2 successes: must close
    cb.record_success();
    assert_eq!(
        cb.state(),
        CircuitState::Closed,
        "exactly at threshold must close"
    );
}

#[test]
fn cb_success_in_closed_resets_failure_count() {
    // Catches mutation that removes `self.failure_count = 0` in Closed arm
    let cfg = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(cfg);

    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.failure_count(), 2);

    cb.record_success();
    assert_eq!(
        cb.failure_count(),
        0,
        "success in closed state must reset failure_count"
    );

    // Now 2 more failures won't open (need 3 from 0)
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn cb_rejection_counter_only_increments_on_reject() {
    // Catches mutation where total_rejections increments unconditionally
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(cfg);

    // Allowed call: no rejection
    assert_eq!(cb.call_allowed(), CallResult::Allowed);
    assert_eq!(cb.total_rejections(), 0);

    cb.record_failure(); // opens circuit

    // Rejected call: rejection count increments
    assert_eq!(cb.call_allowed(), CallResult::Rejected);
    assert_eq!(cb.total_rejections(), 1);
    assert_eq!(cb.total_calls(), 2);
}

#[test]
fn cb_rejection_rate_returns_exact_fraction() {
    // Catches mutation on the division formula or the zero-guard
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(cfg);

    // 1 allowed, then open, then 3 rejected = 4 total, 3 rejected
    cb.call_allowed();
    cb.record_failure();
    cb.call_allowed();
    cb.call_allowed();
    cb.call_allowed();

    assert_eq!(cb.total_calls(), 4);
    assert_eq!(cb.total_rejections(), 3);
    let rate = cb.rejection_rate();
    assert!(
        (rate - 0.75).abs() < f64::EPSILON,
        "expected 0.75, got {rate}"
    );
}

#[test]
fn cb_half_open_failure_immediately_reopens() {
    // Catches mutation that removes the HalfOpen → Open transition on failure
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 5,
        timeout: Duration::from_millis(10),
    };
    let mut cb = CircuitBreaker::new(cfg);
    cb.record_failure();
    std::thread::sleep(Duration::from_millis(20));
    let _ = cb.call_allowed(); // → HalfOpen
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    cb.record_failure();
    assert_eq!(
        cb.state(),
        CircuitState::Open,
        "failure in half-open must reopen"
    );
}

// ── Calibration: normalize boundary & arithmetic mutations ───────────────

#[test]
fn normalize_at_exact_center_returns_zero() {
    // Catches >= vs > on `raw >= self.raw_center`
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    assert_eq!(cal.normalize(500), 0.0, "exactly at center must be 0.0");
}

#[test]
fn normalize_one_above_center_is_positive() {
    // Catches off-by-one and sign errors in the positive branch
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    let v = cal.normalize(501);
    assert!(v > 0.0, "one above center must be positive, got {v}");
    assert!(v < 1.0, "one above center must be less than 1.0, got {v}");
}

#[test]
fn normalize_one_below_center_is_negative() {
    // Catches sign flip or wrong branch selection
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    let v = cal.normalize(499);
    assert!(v < 0.0, "one below center must be negative, got {v}");
    assert!(
        v > -1.0,
        "one below center must be greater than -1.0, got {v}"
    );
}

#[test]
fn normalize_at_min_returns_neg_one() {
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    let v = cal.normalize(0);
    assert!(
        (v - (-1.0)).abs() < 1e-5,
        "at raw_min must be -1.0, got {v}"
    );
}

#[test]
fn normalize_at_max_returns_one() {
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    let v = cal.normalize(1000);
    assert!((v - 1.0).abs() < 1e-5, "at raw_max must be 1.0, got {v}");
}

#[test]
fn normalize_beyond_max_clamps_to_one() {
    // Catches removal of .clamp(0.0, 1.0)
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    let v = cal.normalize(1500);
    assert_eq!(v, 1.0, "beyond max must clamp to 1.0, got {v}");
}

#[test]
fn normalize_beyond_min_clamps_to_neg_one() {
    // Catches removal of .clamp(-1.0, 0.0)
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    let v = cal.normalize(-500);
    assert_eq!(v, -1.0, "beyond min must clamp to -1.0, got {v}");
}

#[test]
fn normalize_zero_range_above_center_returns_zero() {
    // Catches division by zero guard: raw_max == raw_center
    let cal = AxisCalibration::new(0, 0, 500, 500);
    assert_eq!(
        cal.normalize(600),
        0.0,
        "zero positive range must return 0.0"
    );
}

#[test]
fn normalize_zero_range_below_center_returns_zero() {
    // Catches division by zero guard: raw_center == raw_min
    let cal = AxisCalibration::new(0, 500, 1000, 500);
    assert_eq!(
        cal.normalize(400),
        0.0,
        "zero negative range must return 0.0"
    );
}

// ── Error Catalog: return value mutations ────────────────────────────────

#[test]
fn catalog_lookup_returns_correct_code_not_first_entry() {
    // Catches mutation where lookup always returns the first entry
    let last = ErrorCatalog::lookup("INT-004").unwrap();
    assert_eq!(last.code, "INT-004");
    assert_eq!(last.category, ErrorCategory::Internal);

    let mid = ErrorCatalog::lookup("PLG-003").unwrap();
    assert_eq!(mid.code, "PLG-003");
    assert_eq!(mid.category, ErrorCategory::Plugin);
}

#[test]
fn catalog_by_category_returns_exact_count() {
    // Catches filter mutation (== vs !=) or wrong category
    let devices = ErrorCatalog::by_category(ErrorCategory::Device);
    assert_eq!(devices.len(), 5, "Device category should have exactly 5");

    let sims = ErrorCatalog::by_category(ErrorCategory::Sim);
    assert_eq!(sims.len(), 4, "Sim category should have exactly 4");

    let internal = ErrorCatalog::by_category(ErrorCategory::Internal);
    assert_eq!(internal.len(), 4, "Internal category should have exactly 4");
}

#[test]
fn catalog_total_entry_count_is_exact() {
    // Catches mutation that adds/removes entries
    assert_eq!(
        ErrorCatalog::all().len(),
        33,
        "Catalog must have exactly 33 entries"
    );
}

#[test]
fn format_error_unknown_contains_code() {
    // Catches mutation where unknown branch doesn't include the code
    let s = ErrorCatalog::format_error("FAKE-999");
    assert!(s.contains("FAKE-999"), "unknown error must echo the code");
    assert!(s.contains("Unknown"), "unknown error must say Unknown");
}

#[test]
fn format_error_known_contains_all_fields() {
    // Catches mutation that omits description or resolution
    let s = ErrorCatalog::format_error("DEV-001");
    assert!(s.contains("DEV-001"));
    assert!(s.contains("Device not found"));
    assert!(s.contains("Reconnect"));
    assert!(s.contains("Resolution:"));
}
