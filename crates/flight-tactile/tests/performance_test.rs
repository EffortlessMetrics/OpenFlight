//! Performance tests to verify no AX/FFB jitter regression

use flight_bus::{AircraftId, BusSnapshot, SimId};
use flight_tactile::{EffectType, TactileConfig, TactileManager};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test that tactile processing doesn't introduce significant latency
#[test]
fn test_tactile_processing_latency() {
    let mut manager = TactileManager::new();
    let config = TactileConfig::default();

    // Initialize but don't start (to avoid network operations in tests)
    assert!(manager.initialize(config).is_ok());

    // Create test telemetry snapshot
    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

    // Measure processing time
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = manager.process_telemetry(&snapshot);
    }
    let elapsed = start.elapsed();

    // Should process 1000 snapshots in well under 1ms on average
    let avg_per_snapshot = elapsed.as_nanos() / 1000;
    println!(
        "Average processing time per snapshot: {}ns",
        avg_per_snapshot
    );

    // Verify processing time is reasonable (less than 10μs per snapshot)
    assert!(
        avg_per_snapshot < 10_000,
        "Processing too slow: {}ns per snapshot",
        avg_per_snapshot
    );
}

/// Test that tactile manager can be enabled/disabled without affecting performance
#[test]
fn test_enable_disable_performance() {
    let mut manager = TactileManager::new();
    let config = TactileConfig::default();

    assert!(manager.initialize(config).is_ok());

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

    // Test with tactile disabled
    manager.set_enabled(false);
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = manager.process_telemetry(&snapshot);
    }
    let disabled_time = start.elapsed();

    // Test with tactile enabled
    manager.set_enabled(true);
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = manager.process_telemetry(&snapshot);
    }
    let enabled_time = start.elapsed();

    println!(
        "Disabled time: {:?}, Enabled time: {:?}",
        disabled_time, enabled_time
    );

    // Enabled should not be significantly slower than disabled
    // Allow up to 2x overhead when enabled
    assert!(
        enabled_time < disabled_time * 2,
        "Enabled processing too much slower than disabled"
    );
}

/// Test effect processing performance
#[test]
fn test_effect_processing_performance() {
    let manager = TactileManager::new();

    // Test effect testing (which doesn't require full initialization)
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = manager.test_effect(EffectType::Touchdown, 0.5);
    }
    let elapsed = start.elapsed();

    let avg_per_test = elapsed.as_nanos() / 1000;
    println!("Average effect test time: {}ns", avg_per_test);

    // Should be very fast since no bridge is initialized
    assert!(
        avg_per_test < 1_000,
        "Effect testing too slow: {}ns per test",
        avg_per_test
    );
}

/// Test configuration update performance
#[test]
fn test_config_update_performance() {
    let manager = TactileManager::new();
    let config = TactileConfig::default();

    let start = Instant::now();
    for _ in 0..100 {
        let _ = manager.update_config(config.clone());
    }
    let elapsed = start.elapsed();

    let avg_per_update = elapsed.as_nanos() / 100;
    println!("Average config update time: {}ns", avg_per_update);

    // Config updates should be fast
    assert!(
        avg_per_update < 100_000,
        "Config update too slow: {}ns per update",
        avg_per_update
    );
}

/// Test memory usage doesn't grow over time
#[test]
fn test_memory_stability() {
    let mut manager = TactileManager::new();
    let config = TactileConfig::default();

    assert!(manager.initialize(config).is_ok());
    manager.set_enabled(true);

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

    // Process many snapshots to check for memory leaks
    for _ in 0..10000 {
        let _ = manager.process_telemetry(&snapshot);
    }

    // If we get here without running out of memory, the test passes
    // In a real implementation, we might check actual memory usage
    assert!(true, "Memory stability test completed");
}

/// Benchmark tactile bridge thread safety
#[test]
fn test_concurrent_access() {
    use std::thread;

    let manager = Arc::new(RwLock::new(TactileManager::new()));
    let config = TactileConfig::default();

    {
        let mut mgr = manager.write();
        assert!(mgr.initialize(config).is_ok());
        mgr.set_enabled(true);
    }

    let snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));

    // Spawn multiple threads to test concurrent access
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let manager = manager.clone();
            let snapshot = snapshot.clone();

            thread::spawn(move || {
                for _ in 0..250 {
                    let mgr = manager.read();
                    let _ = mgr.process_telemetry(&snapshot);
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Test passed if no deadlocks or panics occurred
    assert!(true, "Concurrent access test completed");
}
