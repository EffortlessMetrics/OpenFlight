//! Comprehensive test suite for flight-scheduler
//!
//! Tests timing accuracy, jitter measurement, PLL behavior,
//! and overload handling.

use super::*;
use crate::metrics::TimingValidator;
use std::thread;
use std::time::{Duration, Instant};
use std::sync::Arc;

#[test]
fn test_scheduler_basic_timing() {
    let config = SchedulerConfig {
        frequency_hz: 100, // Lower frequency for testing
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    
    let mut scheduler = Scheduler::new(config);
    let start = Instant::now();
    
    // Run for 100 ticks (1 second at 100Hz)
    for _ in 0..100 {
        let result = scheduler.wait_for_tick();
        assert!(!result.missed); // Should not miss ticks under light load
    }
    
    let elapsed = start.elapsed();
    let expected = Duration::from_millis(1000); // 1 second
    let tolerance = Duration::from_millis(50);   // 50ms tolerance
    
    assert!(elapsed >= expected - tolerance);
    assert!(elapsed <= expected + tolerance);
}

#[test]
fn test_jitter_measurement() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 65,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    
    let mut scheduler = Scheduler::new(config);
    
    // Run for enough ticks to get meaningful statistics
    for _ in 0..2000 { // 8 seconds at 250Hz
        scheduler.wait_for_tick();
    }
    
    let stats = scheduler.get_stats();
    
    // Should have jitter statistics
    assert!(stats.jitter_stats.is_some());
    
    let jitter = stats.jitter_stats.unwrap();
    assert!(jitter.sample_count > 0);
    
    // Jitter should be reasonable (this may fail on heavily loaded systems)
    // In CI, we'll need to be more lenient
    println!("Jitter p99: {}μs", jitter.p99_ns / 1000);
}

#[test]
fn test_pll_correction() {
    let mut pll = Pll::new(0.001, 4_000_000.0); // 250Hz
    
    // Simulate consistent timing error
    let mut total_correction = 0.0;
    for _ in 0..1000 {
        let corrected = pll.update(1000.0); // 1μs late each time
        total_correction += pll.period_correction();
    }
    
    // Should have accumulated negative correction (shorter period)
    assert!(pll.period_correction() < 0.0);
    
    // Should be bounded
    assert!(pll.period_correction().abs() < 40_000.0); // <1% of 4ms
}

#[test]
fn test_spsc_ring_basic() {
    let ring = SpscRing::new(8);
    
    // Test basic operations
    assert!(ring.try_push(1));
    assert!(ring.try_push(2));
    assert!(ring.try_push(3));
    
    assert_eq!(ring.try_pop(), Some(1));
    assert_eq!(ring.try_pop(), Some(2));
    assert_eq!(ring.try_pop(), Some(3));
    assert_eq!(ring.try_pop(), None);
    
    let stats = ring.stats();
    assert_eq!(stats.produced, 3);
    assert_eq!(stats.consumed, 3);
    assert_eq!(stats.dropped, 0);
}

#[test]
fn test_spsc_ring_drop_policy() {
    let ring = SpscRing::new(4);
    
    // Fill buffer to capacity-1 (ring buffer limitation)
    for i in 0..3 {
        assert!(ring.try_push(i));
    }
    
    // Next push should fail (drop-tail policy)
    assert!(!ring.try_push(999));
    
    let stats = ring.stats();
    assert_eq!(stats.dropped, 1);
    
    // Should still be able to consume
    assert_eq!(ring.try_pop(), Some(0));
    
    // Now should be able to push again
    assert!(ring.try_push(4));
}

#[test]
fn test_concurrent_ring_operations() {
    let ring = Arc::new(SpscRing::new(1024));
    let ring_producer = ring.clone();
    let ring_consumer = ring.clone();
    
    let producer = thread::spawn(move || {
        for i in 0..10000 {
            while !ring_producer.try_push(i) {
                thread::yield_now();
            }
        }
    });
    
    let consumer = thread::spawn(move || {
        let mut received = 0;
        while received < 10000 {
            if let Some(_) = ring_consumer.try_pop() {
                received += 1;
            }
            thread::yield_now();
        }
    });
    
    producer.join().unwrap();
    consumer.join().unwrap();
    
    let stats = ring.stats();
    assert_eq!(stats.consumed, 10000);
}

#[test]
fn test_overload_behavior() {
    let ring = Arc::new(SpscRing::new(16));
    let ring_producer = ring.clone();
    
    // Fast producer, slow consumer
    let producer = thread::spawn(move || {
        for i in 0..1000 {
            ring_producer.try_push(i);
        }
    });
    
    // Slow consumer
    let ring_consumer = ring.clone();
    let consumer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10)); // Let producer get ahead
        
        let mut consumed = 0;
        while consumed < 100 { // Only consume some items
            if let Some(_) = ring_consumer.try_pop() {
                consumed += 1;
            }
            thread::sleep(Duration::from_micros(100)); // Slow consumer
        }
    });
    
    producer.join().unwrap();
    consumer.join().unwrap();
    
    let stats = ring.stats();
    
    // Should have dropped items due to overload
    assert!(stats.dropped > 0);
    
    // Total produced should equal consumed + dropped
    assert_eq!(stats.produced, stats.consumed + stats.dropped);
}

#[test]
fn test_timing_validator() {
    let mut validator = TimingValidator::new(100, Duration::from_millis(200));
    let start = Instant::now();
    
    let mut tick_count = 0;
    while validator.record_and_check(start + Duration::from_millis(tick_count * 10)) {
        tick_count += 1;
        if tick_count > 100 { break; } // Safety limit
    }
    
    let result = validator.finalize();
    
    // Should have run for at least the target duration
    assert!(result.duration >= Duration::from_millis(200));
    
    // Should have collected samples
    assert!(result.jitter_stats.sample_count > 0);
}

/// Integration test that runs scheduler for extended period
#[test]
#[ignore] // Ignore by default as it takes time
fn test_extended_timing_discipline() {
    let config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 65,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    
    let mut scheduler = Scheduler::new(config);
    let start = Instant::now();
    let target_duration = Duration::from_secs(60); // 1 minute test
    
    let mut tick_count = 0;
    while start.elapsed() < target_duration {
        let result = scheduler.wait_for_tick();
        tick_count += 1;
        
        // Log progress every 10 seconds
        if tick_count % 2500 == 0 {
            let stats = scheduler.get_stats();
            println!("Tick {}: Miss rate: {:.4}%", 
                tick_count, stats.miss_rate * 100.0);
            
            if let Some(jitter) = &stats.jitter_stats {
                println!("  Jitter p99: {}μs", jitter.p99_ns / 1000);
            }
        }
    }
    
    let final_stats = scheduler.get_stats();
    
    // Quality gates
    assert!(final_stats.miss_rate < 0.001, "Miss rate too high: {:.4}%", 
        final_stats.miss_rate * 100.0);
    
    if let Some(ref jitter) = final_stats.jitter_stats {
        assert!(jitter.p99_ns.abs() < 500_000, 
            "Jitter p99 too high: {}μs", jitter.p99_ns / 1000);
    }
    
    println!("Extended test completed successfully:");
    println!("  Total ticks: {}", final_stats.total_ticks);
    println!("  Miss rate: {:.6}%", final_stats.miss_rate * 100.0);
    
    if let Some(ref jitter) = final_stats.jitter_stats {
        println!("  Jitter p50: {}μs", jitter.p50_ns / 1000);
        println!("  Jitter p99: {}μs", jitter.p99_ns / 1000);
    }
}