//! Targeted mutation-killing tests for flight-core.
//!
//! Each test asserts specific values so that any mutant changing a constant,
//! swapping an operator, or altering a threshold is caught.

use flight_core::circuit_breaker::{CallResult, CircuitBreaker, CircuitBreakerConfig, CircuitState};
use flight_core::error_catalog::{ErrorCatalog, ErrorCategory};
use flight_core::units::{angles, conversions};
use std::time::Duration;

/// Check that `code` matches the pattern `AAA-NNN` (3 uppercase ASCII + hyphen + 3 digits).
fn matches_code_format(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 7
        && bytes[0].is_ascii_uppercase()
        && bytes[1].is_ascii_uppercase()
        && bytes[2].is_ascii_uppercase()
        && bytes[3] == b'-'
        && bytes[4].is_ascii_digit()
        && bytes[5].is_ascii_digit()
        && bytes[6].is_ascii_digit()
}

// ── 1. Error catalog: exact count, format, and category mapping ─────────────

#[test]
fn error_code_uniqueness_exact() {
    let all = ErrorCatalog::all();

    // Exact count: DEV has 5, the other 7 categories have 4 each → 33
    assert_eq!(all.len(), 33, "catalog must contain exactly 33 error codes");

    // Every code matches the XXX-NNN format
    for info in all {
        assert!(
            matches_code_format(info.code),
            "code {:?} does not match XXX-NNN format",
            info.code
        );
    }

    // All codes are unique
    let mut seen = std::collections::HashSet::new();
    for info in all {
        assert!(seen.insert(info.code), "duplicate code: {}", info.code);
    }

    // Spot-check: every code round-trips through lookup with correct category
    let expected_prefixes: &[(&str, ErrorCategory)] = &[
        ("DEV", ErrorCategory::Device),
        ("SIM", ErrorCategory::Sim),
        ("PRF", ErrorCategory::Profile),
        ("SVC", ErrorCategory::Service),
        ("PLG", ErrorCategory::Plugin),
        ("NET", ErrorCategory::Network),
        ("CFG", ErrorCategory::Config),
        ("INT", ErrorCategory::Internal),
    ];

    for info in all {
        let looked_up = ErrorCatalog::lookup(info.code)
            .unwrap_or_else(|| panic!("lookup failed for {}", info.code));
        assert_eq!(looked_up.code, info.code);
        assert_eq!(looked_up.category, info.category);
    }

    // Verify prefix → category mapping for every single entry
    for info in all {
        let prefix = &info.code[..3];
        let expected_cat = expected_prefixes
            .iter()
            .find(|(p, _)| *p == prefix)
            .unwrap_or_else(|| panic!("unknown prefix: {prefix}"))
            .1;
        assert_eq!(
            info.category, expected_cat,
            "code {} has wrong category: expected {:?}, got {:?}",
            info.code, expected_cat, info.category
        );
    }

    // Verify exact counts per category
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Device).len(), 5);
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Sim).len(), 4);
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Profile).len(), 4);
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Service).len(), 4);
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Plugin).len(), 4);
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Network).len(), 4);
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Config).len(), 4);
    assert_eq!(ErrorCatalog::by_category(ErrorCategory::Internal).len(), 4);
}

// ── 2. Circuit breaker: exact state transitions ─────────────────────────────

#[test]
fn circuit_breaker_state_transitions_exact() {
    let config = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(config);

    // Initial state must be Closed
    assert_eq!(cb.state(), CircuitState::Closed);
    assert_eq!(cb.failure_count(), 0);

    // 2 failures: still Closed (threshold is 3)
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Closed, "must stay Closed after 2 failures");
    assert_eq!(cb.failure_count(), 2);

    // 3rd failure: must transition to Open
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open, "must be Open after 3 failures");
    assert_eq!(cb.failure_count(), 3);

    // Calls must be rejected while Open
    assert_eq!(cb.call_allowed(), CallResult::Rejected);
}

// ── 3. Circuit breaker: HalfOpen → Closed requires full success threshold ───

#[test]
fn circuit_breaker_half_open_to_closed_needs_threshold() {
    let config = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 2,
        timeout: Duration::from_millis(10),
    };
    let mut cb = CircuitBreaker::new(config);

    // Trip to Open with a single failure
    cb.record_failure();
    assert_eq!(cb.state(), CircuitState::Open);

    // Wait past timeout so next call_allowed transitions to HalfOpen
    std::thread::sleep(Duration::from_millis(20));

    let result = cb.call_allowed();
    assert_eq!(result, CallResult::Allowed, "should allow after timeout");
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    // 1 success: NOT enough (threshold=2), must stay HalfOpen
    cb.record_success();
    assert_eq!(
        cb.state(),
        CircuitState::HalfOpen,
        "must stay HalfOpen after only 1 success (threshold=2)"
    );

    // 2nd success: NOW should transition to Closed
    cb.record_success();
    assert_eq!(
        cb.state(),
        CircuitState::Closed,
        "must be Closed after 2 successes in HalfOpen"
    );

    // Verify failure count was reset
    assert_eq!(cb.failure_count(), 0);
}

// ── 4. Unit conversions: exact expected values ──────────────────────────────

#[test]
fn unit_conversion_accuracy_specific_values() {
    // knots_to_mps: 100 kt × 0.514444 = 51.4444 m/s
    let mps = conversions::knots_to_mps(100.0);
    assert!(
        (mps - 51.4444).abs() < 0.01,
        "knots_to_mps(100.0) = {mps}, expected ≈51.4444"
    );

    // feet_to_meters: 1000 ft × 0.3048 = 304.8 m
    let m = conversions::feet_to_meters(1000.0);
    assert!(
        (m - 304.8).abs() < 0.01,
        "feet_to_meters(1000.0) = {m}, expected ≈304.8"
    );

    // mps_to_fpm: 1.0 m/s × 196.85 = 196.85 ft/min
    let fpm = conversions::mps_to_fpm(1.0);
    assert!(
        (fpm - 196.85).abs() < 0.01,
        "mps_to_fpm(1.0) = {fpm}, expected ≈196.85"
    );

    // degrees_to_radians: 180° = π
    let rad = conversions::degrees_to_radians(180.0);
    assert!(
        (rad - std::f32::consts::PI).abs() < 0.0001,
        "degrees_to_radians(180.0) = {rad}, expected ≈π"
    );
}

// ── 5. Angle normalization: exact signed/unsigned boundary values ────────────

#[test]
fn angle_normalization_specific_values() {
    // Signed: 270° must wrap to -90° (not +90°)
    let v = angles::normalize_degrees_signed(270.0);
    assert!(
        (v - (-90.0)).abs() < 0.001,
        "normalize_degrees_signed(270.0) = {v}, expected -90.0"
    );

    // Signed: -270° must wrap to +90° (not -90°)
    let v = angles::normalize_degrees_signed(-270.0);
    assert!(
        (v - 90.0).abs() < 0.001,
        "normalize_degrees_signed(-270.0) = {v}, expected 90.0"
    );

    // Signed: 180° must be in [-180, 180]
    let v = angles::normalize_degrees_signed(180.0);
    assert!(
        (-180.0..=180.0).contains(&v),
        "normalize_degrees_signed(180.0) = {v}, must be in [-180, 180]"
    );

    // Unsigned: -90° must become 270° (not 90°)
    let v = angles::normalize_degrees_unsigned(-90.0);
    assert!(
        (v - 270.0).abs() < 0.001,
        "normalize_degrees_unsigned(-90.0) = {v}, expected 270.0"
    );

    // Unsigned: 360° must become 0° (not 360°)
    let v = angles::normalize_degrees_unsigned(360.0);
    assert!(
        v.abs() < 0.001,
        "normalize_degrees_unsigned(360.0) = {v}, expected 0.0"
    );
}
