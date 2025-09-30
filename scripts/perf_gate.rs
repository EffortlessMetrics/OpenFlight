#!/usr/bin/env cargo +nightly -Zscript
//! CI Performance Gate Script
//! 
//! Runs performance tests and fails the build if timing regressions are detected.
//! 
//! Usage: cargo run --bin perf_gate [--quick]

use std::process;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick_mode = args.contains(&"--quick".to_string());
    
    println!("🚀 Flight Hub Performance Gate");
    println!("==============================");
    
    if quick_mode {
        println!("Running in QUICK mode (suitable for PR checks)");
        run_quick_tests();
    } else {
        println!("Running FULL performance validation");
        run_full_tests();
    }
}

fn run_quick_tests() {
    println!("\n📊 Running quick performance tests...");
    
    // Run basic scheduler tests
    let result = std::process::Command::new("cargo")
        .args(&["test", "--package", "flight-scheduler", "--lib", 
               "test_scheduler_basic_timing", "--", "--nocapture"])
        .status()
        .expect("Failed to run scheduler tests");
    
    if !result.success() {
        eprintln!("❌ Scheduler timing tests FAILED");
        process::exit(1);
    }
    
    // Run virtual device tests
    let result = std::process::Command::new("cargo")
        .args(&["test", "--package", "flight-virtual", "--lib",
               "test_ci_benchmark", "--", "--nocapture"])
        .status()
        .expect("Failed to run virtual device tests");
    
    if !result.success() {
        eprintln!("❌ Virtual device tests FAILED");
        process::exit(1);
    }
    
    println!("✅ Quick performance gate PASSED");
}

fn run_full_tests() {
    println!("\n📊 Running comprehensive performance validation...");
    
    // Run all scheduler tests
    let result = std::process::Command::new("cargo")
        .args(&["test", "--package", "flight-scheduler", "--lib", "--", "--nocapture"])
        .status()
        .expect("Failed to run scheduler tests");
    
    if !result.success() {
        eprintln!("❌ Scheduler tests FAILED");
        process::exit(1);
    }
    
    // Run all virtual device tests
    let result = std::process::Command::new("cargo")
        .args(&["test", "--package", "flight-virtual", "--lib", "--", "--nocapture"])
        .status()
        .expect("Failed to run virtual device tests");
    
    if !result.success() {
        eprintln!("❌ Virtual device tests FAILED");
        process::exit(1);
    }
    
    // Run integration tests
    let result = std::process::Command::new("cargo")
        .args(&["test", "--package", "flight-virtual", "--lib",
               "test_scheduler_virtual_device_integration", "--", "--nocapture"])
        .status()
        .expect("Failed to run integration tests");
    
    if !result.success() {
        eprintln!("❌ Integration tests FAILED");
        process::exit(1);
    }
    
    println!("✅ Full performance gate PASSED");
    
    // Extract and display metrics
    display_performance_metrics();
}

fn display_performance_metrics() {
    println!("\n📈 Performance Metrics Summary:");
    println!("==============================");
    
    if let Ok(jitter) = std::env::var("FLIGHT_HUB_JITTER_P99_US") {
        println!("  Jitter p99: {}μs", jitter);
    }
    
    if let Ok(hid_latency) = std::env::var("FLIGHT_HUB_HID_P99_US") {
        println!("  HID Latency p99: {}μs", hid_latency);
    }
    
    if let Ok(miss_rate) = std::env::var("FLIGHT_HUB_MISS_RATE") {
        if let Ok(rate) = miss_rate.parse::<f64>() {
            println!("  Miss Rate: {:.3}%", rate * 100.0);
        }
    }
    
    println!("\n💡 These metrics are tracked for performance regression detection");
}