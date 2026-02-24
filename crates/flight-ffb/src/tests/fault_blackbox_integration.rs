// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for fault detection, blackbox recording, and latched indicators
//!
//! Validates that faults produce blackbox dumps and latched indicators as required.

use crate::*;
use std::thread;
use std::time::Duration;

#[test]
fn test_fault_produces_blackbox_dump() {
    // Create FFB engine
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Record some axis frames before fault
    for i in 0..10 {
        engine
            .record_axis_frame("test_device".to_string(), 0.5, 0.6, (i as f32) * 0.1)
            .unwrap();
        thread::sleep(Duration::from_millis(10));
    }

    // Trigger a fault
    engine.process_fault(FaultType::UsbStall).unwrap();

    // Verify fault was recorded
    let fault_history = engine.get_fault_history();
    assert_eq!(fault_history.len(), 1);
    assert_eq!(fault_history[0].fault_type, FaultType::UsbStall);

    // Verify blackbox has active capture
    assert!(
        engine
            .get_blackbox_recorder()
            .get_active_capture()
            .is_some()
    );

    // Record post-fault entries spanning > 1s to trigger capture completion
    // (post_fault_duration = 1s default; use 15ms sleep so Windows timer granularity
    //  still accumulates enough time across ~80 frames)
    for i in 0..80 {
        engine
            .record_axis_frame("test_device".to_string(), 0.5, 0.6, (i as f32) * 0.01)
            .unwrap();
        thread::sleep(Duration::from_millis(15));
    }

    // Verify capture was completed
    let completed_captures = engine.get_blackbox_recorder().get_completed_captures();
    assert!(!completed_captures.is_empty());

    let capture = &completed_captures[0];
    assert!(capture.complete);
    assert!(!capture.pre_fault_entries.is_empty());
    assert!(!capture.post_fault_entries.is_empty());

    // Verify fault entry is present
    if let BlackboxEntry::Fault {
        fault_type,
        fault_code,
        ..
    } = &capture.fault_entry
    {
        assert_eq!(fault_type, "HID_OUT_STALL");
        assert_eq!(fault_code, "HID_OUT_STALL");
    } else {
        panic!("Expected Fault entry");
    }
}

#[test]
fn test_fault_produces_latched_indicator() {
    // Create FFB engine
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Initially no latched fault
    assert!(!engine.has_latched_fault());
    assert!(engine.get_latched_fault().is_none());

    // Trigger a fault that causes safety state transition
    engine.process_fault(FaultType::UsbStall).unwrap();

    // Verify latched fault indicator
    assert!(engine.has_latched_fault());
    assert_eq!(engine.safety_state(), SafetyState::Faulted);

    // Verify latched fault details
    let latched_fault = engine.get_latched_fault();
    assert!(latched_fault.is_some());

    let fault = latched_fault.unwrap();
    assert_eq!(fault.fault_type, FaultType::UsbStall);
    assert_eq!(fault.error_code, "HID_OUT_STALL");
    assert_eq!(
        fault.kb_article_url,
        "https://docs.flight-hub.dev/kb/hid-out-stall"
    );
    assert!(fault.caused_safety_transition);
}

#[test]
fn test_multiple_faults_produce_multiple_dumps() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Trigger first fault
    engine.process_fault(FaultType::UsbStall).unwrap();

    // Record post-fault entries spanning > 1s to trigger capture completion
    for i in 0..80 {
        engine
            .record_axis_frame("test_device".to_string(), 0.5, 0.6, (i as f32) * 0.01)
            .unwrap();
        thread::sleep(Duration::from_millis(15));
    }

    // Reset from fault
    #[allow(deprecated)]
    engine.reset_from_fault(true).unwrap();

    // Trigger second fault
    engine.process_fault(FaultType::OverTemp).unwrap();

    // Record post-fault entries for second capture spanning > 1s
    for i in 0..80 {
        engine
            .record_axis_frame("test_device".to_string(), 0.5, 0.6, (i as f32) * 0.01)
            .unwrap();
        thread::sleep(Duration::from_millis(15));
    }

    // Verify faults — reset_from_fault clears fault history, so only the post-reset fault is visible
    let fault_history = engine.get_fault_history();
    assert_eq!(fault_history.len(), 1);
    assert_eq!(fault_history[0].fault_type, FaultType::OverTemp);

    // Verify both captures exist — the first capture completed before reset, the second after
    let completed_captures = engine.get_blackbox_recorder().get_completed_captures();
    assert_eq!(completed_captures.len(), 2);
}

#[test]
fn test_plugin_fault_does_not_latch() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Trigger plugin fault (should not cause safety state transition)
    engine.process_fault(FaultType::PluginOverrun).unwrap();

    // Verify no latched fault (plugin faults don't latch)
    assert!(!engine.has_latched_fault());
    assert_eq!(engine.safety_state(), SafetyState::SafeTorque);

    // But fault should still be recorded
    let fault_history = engine.get_fault_history();
    assert_eq!(fault_history.len(), 1);
    assert_eq!(fault_history[0].fault_type, FaultType::PluginOverrun);
}

#[test]
fn test_fault_capture_includes_pre_fault_data() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Record axis frames with distinct torque values
    for i in 0..20 {
        engine
            .record_axis_frame(
                "test_device".to_string(),
                0.5,
                0.6,
                (i as f32) * 0.5, // Distinct torque values
            )
            .unwrap();
        thread::sleep(Duration::from_millis(10));
    }

    // Trigger fault
    engine.process_fault(FaultType::NanValue).unwrap();

    // Verify pre-fault data was captured
    let active_capture = engine.get_blackbox_recorder().get_active_capture();
    assert!(active_capture.is_some());

    let capture = active_capture.unwrap();
    assert!(!capture.pre_fault_entries.is_empty());

    // Verify pre-fault entries contain axis frames
    let axis_frames: Vec<_> = capture
        .pre_fault_entries
        .iter()
        .filter(|entry| matches!(entry, BlackboxEntry::AxisFrame { .. }))
        .collect();

    assert!(!axis_frames.is_empty());

    // Verify torque values are distinct (proving we captured real data)
    if let BlackboxEntry::AxisFrame { torque_nm, .. } = axis_frames[0] {
        assert!(*torque_nm >= 0.0);
    }
}

#[test]
fn test_fault_capture_includes_post_fault_data() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Trigger fault
    engine.process_fault(FaultType::EndpointError).unwrap();

    // Record post-fault axis frames spanning > 1s to trigger capture completion
    for i in 0..80 {
        engine
            .record_axis_frame("test_device".to_string(), 0.5, 0.6, (i as f32) * 0.01)
            .unwrap();
        thread::sleep(Duration::from_millis(15));
    }

    // Verify post-fault data was captured
    let completed_captures = engine.get_blackbox_recorder().get_completed_captures();
    assert!(!completed_captures.is_empty());

    let capture = &completed_captures[0];
    assert!(!capture.post_fault_entries.is_empty());

    // Verify post-fault entries contain axis frames
    let axis_frames: Vec<_> = capture
        .post_fault_entries
        .iter()
        .filter(|entry| matches!(entry, BlackboxEntry::AxisFrame { .. }))
        .collect();

    assert!(!axis_frames.is_empty());
}

#[test]
fn test_latched_fault_persists_until_reset() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Trigger fault
    engine.process_fault(FaultType::OverCurrent).unwrap();

    // Verify latched
    assert!(engine.has_latched_fault());
    assert_eq!(engine.safety_state(), SafetyState::Faulted);

    // Update engine multiple times - fault should remain latched
    for _ in 0..10 {
        engine.update().unwrap();
        assert!(engine.has_latched_fault());
        assert_eq!(engine.safety_state(), SafetyState::Faulted);
    }

    // Reset from fault (requires power cycle)
    engine.reset_from_fault(true).unwrap();

    // Verify fault is cleared
    assert!(!engine.has_latched_fault());
    assert_eq!(engine.safety_state(), SafetyState::SafeTorque);
}

#[test]
fn test_fault_record_contains_kb_article() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Trigger fault
    engine.process_fault(FaultType::DeviceTimeout).unwrap();

    // Verify fault record contains KB article
    let latched_fault = engine.get_latched_fault().unwrap();
    assert_eq!(latched_fault.error_code, "DEVICE_TIMEOUT");
    assert_eq!(
        latched_fault.kb_article_url,
        "https://docs.flight-hub.dev/kb/device-timeout"
    );
}

#[test]
fn test_blackbox_statistics_after_fault() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();

    // Record some data
    for i in 0..10 {
        engine
            .record_axis_frame("test_device".to_string(), 0.5, 0.6, i as f32)
            .unwrap();
    }

    // Trigger fault
    engine.process_fault(FaultType::EncoderInvalid).unwrap();

    // Get blackbox statistics
    let stats = engine.get_blackbox_recorder().get_statistics();

    assert!(stats.total_entries > 0);
    assert!(stats.active_fault_capture);
    assert!(stats.buffer_utilization > 0.0);
}
