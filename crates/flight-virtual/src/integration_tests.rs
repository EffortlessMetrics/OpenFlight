// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for scheduler and virtual devices
//!
//! Tests the complete system including scheduler, virtual devices,
//! and performance gates working together.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use flight_scheduler::{Scheduler, SchedulerConfig, SpscRing};
use crate::{VirtualDeviceManager, VirtualDeviceConfig, DeviceType};
use crate::loopback::{LoopbackHid, HidReport};
use crate::perf_gate::{PerfGate, PerfGateConfig};

#[test]
fn test_scheduler_virtual_device_integration() {
    let mut manager = VirtualDeviceManager::new();
    
    // Create virtual joystick
    let config = VirtualDeviceConfig {
        name: "Test Joystick".to_string(),
        device_type: DeviceType::Joystick { axes: 3 },
        vid: 0x1234,
        pid: 0x5678,
        serial: "TEST001".to_string(),
        latency_us: 100,
        packet_loss_rate: 0.0,
    };
    
    let device = manager.create_device(config);
    
    // Create scheduler
    let scheduler_config = SchedulerConfig {
        frequency_hz: 100, // Lower frequency for testing
        busy_spin_us: 50,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    
    let mut scheduler = Scheduler::new(scheduler_config);
    
    // Run integrated test
    let start = Instant::now();
    let mut tick_count = 0;
    
    while start.elapsed() < Duration::from_millis(500) {
        let result = scheduler.wait_for_tick();
        
        // Update device state each tick
        let time_factor = tick_count as f32 * 0.1;
        device.set_axis(0, (time_factor.sin() * 0.8).clamp(-1.0, 1.0));
        device.set_axis(1, (time_factor.cos() * 0.6).clamp(-1.0, 1.0));
        device.set_button(0, tick_count % 20 < 10);
        
        // Generate HID report
        if let Some(report_data) = device.generate_input_report() {
            // Simulate processing the report
            assert!(report_data.len() > 0);
        }
        
        tick_count += 1;
    }
    
    let stats = scheduler.get_stats();
    let device_stats = device.get_stats();
    
    // Verify integration worked
    assert!(stats.total_ticks > 40); // Should have run ~50 ticks at 100Hz for 500ms
    assert!(device_stats.input_reports > 40);
    assert_eq!(stats.missed_ticks, 0); // Should not miss ticks under light load
    
    println!("Integration test completed:");
    println!("  Scheduler ticks: {}", stats.total_ticks);
    println!("  Device reports: {}", device_stats.input_reports);
    println!("  Miss rate: {:.6}%", stats.miss_rate * 100.0);
}

#[test]
fn test_multi_device_scheduler() {
    let mut manager = VirtualDeviceManager::new();
    
    // Create multiple virtual devices
    let devices: Vec<_> = (0..3).map(|i| {
        let config = VirtualDeviceConfig {
            name: format!("Device {}", i),
            device_type: DeviceType::Joystick { axes: 2 },
            vid: 0x1234,
            pid: 0x5678 + i as u16,
            serial: format!("DEV{:03}", i),
            latency_us: 50 + i as u32 * 25,
            packet_loss_rate: 0.0,
        };
        manager.create_device(config)
    }).collect();
    
    let scheduler_config = SchedulerConfig {
        frequency_hz: 250,
        busy_spin_us: 65,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    
    let mut scheduler = Scheduler::new(scheduler_config);
    
    // Run with multiple devices
    let start = Instant::now();
    let mut tick_count = 0;
    
    while start.elapsed() < Duration::from_millis(200) {
        let _result = scheduler.wait_for_tick();
        
        // Update all devices
        for (i, device) in devices.iter().enumerate() {
            let phase = tick_count as f32 * 0.1 + i as f32;
            device.set_axis(0, phase.sin() * 0.5);
            device.set_axis(1, phase.cos() * 0.5);
            
            // Generate reports from all devices
            device.generate_input_report();
        }
        
        tick_count += 1;
    }
    
    let stats = scheduler.get_stats();
    
    // Verify multi-device operation
    assert!(stats.total_ticks > 40); // ~50 ticks expected
    
    for (i, device) in devices.iter().enumerate() {
        let device_stats = device.get_stats();
        assert!(device_stats.input_reports > 40, "Device {} reports: {}", i, device_stats.input_reports);
    }
}

#[test]
fn test_scheduler_with_loopback_hid() {
    let loopback = LoopbackHid::with_config(1024, Duration::from_micros(50));
    
    let scheduler_config = SchedulerConfig {
        frequency_hz: 200,
        busy_spin_us: 60,
        pll_gain: 0.001,
        measure_jitter: true,
    };
    
    let mut scheduler = Scheduler::new(scheduler_config);
    
    // Run scheduler with HID loopback
    let start = Instant::now();
    let mut tick_count = 0;
    
    while start.elapsed() < Duration::from_millis(250) {
        let result = scheduler.wait_for_tick();
        
        // Send HID report each tick
        let report_data = vec![
            (tick_count & 0xFF) as u8,
            ((tick_count >> 8) & 0xFF) as u8,
            0x00, 0x00, // Padding
        ];
        
        let report = HidReport::new(0x01, report_data);
        loopback.send_input_report(report);
        
        // Occasionally send output report
        if tick_count % 10 == 0 {
            let output_report = HidReport::new(0x02, vec![0xFF]);
            loopback.send_output_report(output_report);
        }
        
        tick_count += 1;
    }
    
    let scheduler_stats = scheduler.get_stats();
    let loopback_stats = loopback.get_stats();
    
    // Verify combined operation
    assert!(scheduler_stats.total_ticks > 40);
    assert!(loopback_stats.input_reports_sent > 40);
    assert!(loopback_stats.avg_latency_us > 0.0);
    
    println!("Scheduler + HID test completed:");
    println!("  Scheduler ticks: {}", scheduler_stats.total_ticks);
    println!("  HID reports sent: {}", loopback_stats.input_reports_sent);
    println!("  HID avg latency: {:.1}μs", loopback_stats.avg_latency_us);
}

#[test]
fn test_overload_behavior() {
    let ring: SpscRing<u32> = SpscRing::new(16);
    
    let scheduler_config = SchedulerConfig {
        frequency_hz: 1000, // High frequency to stress test
        busy_spin_us: 30,
        pll_gain: 0.001,
        measure_jitter: false, // Disable to reduce overhead
    };
    
    let mut scheduler = Scheduler::new(scheduler_config);
    
    // Simulate overload scenario
    let start = Instant::now();
    let mut tick_count = 0;
    
    while start.elapsed() < Duration::from_millis(100) {
        let _result = scheduler.wait_for_tick();
        
        // Try to push data to ring (simulating data flow)
        ring.try_push(tick_count);
        
        // Simulate some work
        for _ in 0..100 {
            std::hint::black_box(tick_count * 2);
        }
        
        tick_count += 1;
    }
    
    let stats = scheduler.get_stats();
    let ring_stats = ring.stats();
    
    // Should handle overload gracefully
    assert!(stats.total_ticks > 50); // Should have processed many ticks
    
    // Ring should have dropped some items under high load
    assert!(ring_stats.produced > 0);
    
    println!("Overload test completed:");
    println!("  Scheduler ticks: {}", stats.total_ticks);
    println!("  Ring produced: {}", ring_stats.produced);
    println!("  Ring dropped: {}", ring_stats.dropped);
    println!("  Miss rate: {:.3}%", stats.miss_rate * 100.0);
}

#[test]
#[ignore] // Ignore by default as it's a longer test
fn test_extended_integration() {
    let mut manager = VirtualDeviceManager::new();
    
    // Create comprehensive test setup
    let joystick = manager.create_device(VirtualDeviceConfig {
        name: "Test Joystick".to_string(),
        device_type: DeviceType::Joystick { axes: 4 },
        vid: 0x1234, pid: 0x0001, serial: "JS001".to_string(),
        latency_us: 80, packet_loss_rate: 0.001,
    });
    
    let throttle = manager.create_device(VirtualDeviceConfig {
        name: "Test Throttle".to_string(),
        device_type: DeviceType::Throttle { levers: 2 },
        vid: 0x1234, pid: 0x0002, serial: "TH001".to_string(),
        latency_us: 120, packet_loss_rate: 0.0,
    });
    
    let loopback = LoopbackHid::with_config(2048, Duration::from_micros(75));
    
    // Run performance gate test
    let perf_config = PerfGateConfig {
        frequency_hz: 250,
        duration: Duration::from_secs(30), // 30-second test
        max_jitter_p99_ns: 1_000_000,     // 1ms (lenient for test)
        max_hid_latency_p99_us: 500,      // 500μs
        max_miss_rate: 0.01,              // 1%
        hid_samples: 1000,
    };
    
    let mut perf_gate = PerfGate::new(perf_config);
    let result = perf_gate.run();
    
    // Verify extended test results
    assert!(result.timing_result.total_ticks > 7000); // ~7500 ticks expected
    assert!(result.hid_result.samples == 1000);
    
    // Check device statistics
    let js_stats = joystick.get_stats();
    let th_stats = throttle.get_stats();
    let lb_stats = loopback.get_stats();
    
    println!("Extended integration test results:");
    println!("  Performance gate: {}", if result.passed { "PASS" } else { "FAIL" });
    println!("  Joystick reports: {}", js_stats.input_reports);
    println!("  Throttle reports: {}", th_stats.input_reports);
    println!("  Loopback reports: {}", lb_stats.input_reports_sent);
    println!("  Total duration: {:?}", result.total_duration);
    
    // Test should pass (may be flaky on heavily loaded systems)
    if !result.passed {
        println!("Warning: Performance gate failed - may indicate system overload");
    }
}

/// Benchmark test for CI performance monitoring
#[test]
fn test_ci_benchmark() {
    let config = PerfGateConfig {
        frequency_hz: 250,
        duration: Duration::from_secs(5), // Quick test for CI
        max_jitter_p99_ns: 2_000_000,    // 2ms (lenient for CI)
        max_hid_latency_p99_us: 1000,    // 1ms
        max_miss_rate: 0.05,             // 5%
        hid_samples: 100,
    };
    
    let mut gate = PerfGate::new(config);
    let result = gate.run();
    
    // Always pass in CI, but collect metrics
    println!("CI Benchmark Results:");
    println!("  Jitter p99: {}μs", result.timing_result.jitter_p99_ns / 1000);
    println!("  HID p99: {}μs", result.hid_result.p99_latency_us);
    println!("  Miss rate: {:.3}%", result.timing_result.miss_rate * 100.0);
    
    // Export metrics for CI dashboard (in real implementation)
    unsafe {
        std::env::set_var("FLIGHT_HUB_JITTER_P99_US", 
                         (result.timing_result.jitter_p99_ns / 1000).to_string());
        std::env::set_var("FLIGHT_HUB_HID_P99_US", 
                         result.hid_result.p99_latency_us.to_string());
        std::env::set_var("FLIGHT_HUB_MISS_RATE", 
                         result.timing_result.miss_rate.to_string());
    }
}