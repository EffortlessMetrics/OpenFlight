//! CI Performance Gate Example
//!
//! Demonstrates how to use flight-tracing for CI performance monitoring
//! and regression detection. This example shows:
//!
//! 1. Setting up tracing for a performance test
//! 2. Running a simulated RT loop with measurements
//! 3. Checking quality gates and detecting regressions
//! 4. Exporting metrics for CI consumption

use flight_tracing::{
    HidWriteTracer, TickTracer,
    counters::CounterSnapshot,
    get_counters, initialize,
    regression::{AlertSeverity, RegressionDetector, Thresholds},
    reset_counters, shutdown, trace_deadline_miss, trace_hid_write, trace_tick_end,
    trace_tick_start,
};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing system
    println!("Initializing Flight Hub tracing...");
    initialize()?;

    // Reset counters for clean test
    reset_counters();

    // Run performance test scenarios
    println!("Running performance test scenarios...");

    // Scenario 1: Good performance (should pass all gates)
    println!("\n=== Scenario 1: Good Performance ===");
    let good_snapshot = run_performance_test("good", 1000, 100_000, 200_000)?;
    check_quality_gates(&good_snapshot);

    // Scenario 2: Marginal performance (should pass but show warnings)
    println!("\n=== Scenario 2: Marginal Performance ===");
    reset_counters();
    let marginal_snapshot = run_performance_test("marginal", 1000, 400_000, 280_000)?;
    check_quality_gates(&marginal_snapshot);

    // Scenario 3: Poor performance (should fail quality gates)
    println!("\n=== Scenario 3: Poor Performance ===");
    reset_counters();
    let poor_snapshot = run_performance_test("poor", 1000, 800_000, 400_000)?;
    check_quality_gates(&poor_snapshot);

    // Demonstrate regression detection
    println!("\n=== Regression Detection ===");
    demonstrate_regression_detection(&good_snapshot, &poor_snapshot)?;

    // Export metrics for CI
    println!("\n=== CI Metrics Export ===");
    export_ci_metrics(&poor_snapshot)?;

    // Cleanup
    shutdown()?;
    println!("\nTracing shutdown complete.");

    Ok(())
}

/// Run a simulated performance test with specified parameters
fn run_performance_test(
    name: &str,
    tick_count: u32,
    target_jitter_ns: u64,
    target_hid_latency_ns: u64,
) -> Result<CounterSnapshot, Box<dyn std::error::Error>> {
    println!(
        "Running {} performance test ({} ticks)...",
        name, tick_count
    );

    let start_time = Instant::now();
    let tick_period = Duration::from_nanos(4_000_000); // 250Hz = 4ms

    for tick in 0..tick_count {
        let tick_start = Instant::now();

        // Start tick tracing
        let _tick_tracer = TickTracer::start(tick as u64);

        // Simulate RT work with some variability
        let work_duration = Duration::from_micros(500 + (tick % 100) as u64);
        thread::sleep(work_duration);

        // Simulate HID write every 10th tick
        if tick % 10 == 0 {
            let _hid_tracer = HidWriteTracer::start(0x1234, 64);

            // Simulate HID write latency
            let hid_latency =
                Duration::from_nanos(target_hid_latency_ns + (tick % 50) as u64 * 1000);
            thread::sleep(hid_latency);
        }

        // Calculate jitter and emit tick end
        let tick_duration = tick_start.elapsed();
        let expected_time = start_time + tick_period * tick;
        let actual_time = tick_start;

        let jitter_ns = if actual_time >= expected_time {
            (actual_time - expected_time).as_nanos() as i64
        } else {
            -((expected_time - actual_time).as_nanos() as i64)
        };

        // Add artificial jitter based on target
        let artificial_jitter = (target_jitter_ns as i64) + (tick as i64 % 100 - 50) * 1000;

        // Simulate deadline miss for poor performance
        if name == "poor" && tick % 50 == 0 {
            trace_deadline_miss!(tick as u64, 2_000_000); // 2ms miss
        }

        // End tick tracing with jitter
        drop(_tick_tracer);
        trace_tick_end!(
            tick as u64,
            tick_duration.as_nanos() as u64,
            artificial_jitter
        );

        // Maintain timing for realistic test
        let next_tick_time = start_time + tick_period * (tick + 1);
        let now = Instant::now();
        if next_tick_time > now {
            thread::sleep(next_tick_time - now);
        }
    }

    // Get final snapshot
    let snapshot = get_counters();
    println!(
        "Test completed: {} ticks, {:.1}s duration",
        snapshot.total_ticks,
        snapshot.session_duration_ms as f64 / 1000.0
    );

    Ok(snapshot)
}

/// Check quality gates and report results
fn check_quality_gates(snapshot: &CounterSnapshot) {
    println!("Quality Gate Results:");
    println!(
        "  Jitter p99: {:.1}μs (limit: 500μs)",
        snapshot.jitter.p99_ns as f64 / 1_000.0
    );
    println!(
        "  HID avg latency: {:.1}μs (limit: 300μs)",
        snapshot.hid.avg_time_ns as f64 / 1_000.0
    );
    println!(
        "  Miss rate: {:.3}% (limit: 1%)",
        snapshot.miss_rate * 100.0
    );
    println!("  Writer drops: {} (limit: 100)", snapshot.writer_drops);

    // Check gates
    let mut violations = Vec::new();

    if snapshot.jitter.sample_count >= 100 && snapshot.jitter.p99_ns.abs() > 500_000 {
        violations.push(format!(
            "Jitter p99 exceeds 500μs: {:.1}μs",
            snapshot.jitter.p99_ns as f64 / 1_000.0
        ));
    }

    if snapshot.hid.avg_time_ns > 300_000 {
        violations.push(format!(
            "HID latency exceeds 300μs: {:.1}μs",
            snapshot.hid.avg_time_ns as f64 / 1_000.0
        ));
    }

    if snapshot.miss_rate > 0.01 {
        violations.push(format!(
            "Miss rate exceeds 1%: {:.3}%",
            snapshot.miss_rate * 100.0
        ));
    }

    if snapshot.writer_drops > 100 {
        violations.push(format!(
            "Writer drops exceed 100: {}",
            snapshot.writer_drops
        ));
    }

    if violations.is_empty() {
        println!("  ✅ All quality gates PASSED");
    } else {
        println!("  ❌ Quality gate FAILURES:");
        for violation in violations {
            println!("    - {}", violation);
        }
    }
}

/// Demonstrate regression detection between two snapshots
fn demonstrate_regression_detection(
    baseline: &CounterSnapshot,
    current: &CounterSnapshot,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Setting up regression detector...");

    let mut detector = RegressionDetector::new();
    detector.add_baseline(baseline.clone());

    let result = detector.check_regression(current.clone());

    println!("Regression Analysis:");
    println!(
        "  Baseline jitter p99: {:.1}μs",
        baseline.jitter.p99_ns as f64 / 1_000.0
    );
    println!(
        "  Current jitter p99: {:.1}μs",
        current.jitter.p99_ns as f64 / 1_000.0
    );

    if let Some(comp) = &result.comparison {
        println!(
            "  Jitter change: {:.1}% ({:+.1}μs)",
            comp.jitter.relative_change * 100.0,
            comp.jitter.absolute_change / 1_000.0
        );
        println!(
            "  HID latency change: {:.1}% ({:+.1}μs)",
            comp.hid_latency.relative_change * 100.0,
            comp.hid_latency.absolute_change / 1_000.0
        );
    }

    if result.regression_detected {
        println!("  🚨 REGRESSION DETECTED");
        for alert in &result.alerts {
            println!("    {}", alert);
        }
    } else {
        println!("  ✅ No regression detected");
    }

    Ok(())
}

/// Export metrics in CI-friendly formats
fn export_ci_metrics(snapshot: &CounterSnapshot) -> Result<(), Box<dyn std::error::Error>> {
    println!("Exporting CI metrics...");

    // JSON export
    let json_output = snapshot.to_json()?;
    println!("\nJSON Metrics:");
    println!("{}", json_output);

    // Key-value pairs for CI systems
    println!("\nKey-Value Metrics:");
    let kv_pairs = snapshot.to_kv_pairs();
    for (key, value) in kv_pairs {
        println!("{}={}", key, value);
    }

    // GitHub Actions format
    println!("\nGitHub Actions Output:");
    println!(
        "::set-output name=jitter_p99_us::{:.1}",
        snapshot.jitter.p99_ns as f64 / 1_000.0
    );
    println!(
        "::set-output name=hid_avg_us::{:.1}",
        snapshot.hid.avg_time_ns as f64 / 1_000.0
    );
    println!(
        "::set-output name=miss_rate_percent::{:.4}",
        snapshot.miss_rate * 100.0
    );

    // Quality gate status
    let jitter_pass = snapshot.jitter.p99_ns.abs() <= 500_000;
    let hid_pass = snapshot.hid.avg_time_ns <= 300_000;
    let miss_pass = snapshot.miss_rate <= 0.01;
    let overall_pass = jitter_pass && hid_pass && miss_pass;

    println!("::set-output name=quality_gates_passed::{}", overall_pass);

    if !overall_pass {
        println!("::error::Quality gates failed - performance regression detected");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ci_integration() {
        // This test verifies the CI integration works without panicking
        // In a real CI environment, this would be run as a separate binary

        // Initialize tracing
        initialize().unwrap();

        // Run a short performance test
        reset_counters();
        let snapshot = run_performance_test("test", 100, 100_000, 200_000).unwrap();

        // Verify we got some data
        assert!(snapshot.total_ticks > 0);
        assert!(snapshot.jitter.sample_count > 0);

        // Test quality gate checking
        check_quality_gates(&snapshot);

        // Test metrics export
        export_ci_metrics(&snapshot).unwrap();

        shutdown().unwrap();
    }
}
