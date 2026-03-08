// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the flight-test-helpers crate.
//!
//! Covers: fake devices, fake sim backends, deterministic clock, trace replay,
//! fixture builders, and assertion helpers.

use flight_test_helpers::assert_helpers::{
    assert_axis_in_range, assert_frequency_within, assert_jitter_under, assert_latency_under,
    assert_no_inf, assert_no_nan, assert_symmetric_deadzone,
};
use flight_test_helpers::{
    assert_approx_eq, assert_bounded_rate, assert_in_range, assert_monotonic, create_temp_dir,
    wait_for_condition, DeterministicClock, DeviceFixtureBuilder, FakeDevice, FakeInput, FakeSim,
    FakeSnapshot, ProfileFixtureBuilder, SnapshotResult, SnapshotStore, TelemetryFixtureBuilder,
    TestConfigBuilder, TestDeviceBuilder, TestHarness, TraceComparator, TraceEvent,
    TraceEventType, TracePlayer, TraceRecording, TraceSource,
};
use std::time::Duration;

/// 250 Hz tick period in microseconds.
const TICK_US: u64 = 4_000;

// ===========================================================================
// 1. Fake devices (6 tests)
// ===========================================================================

#[test]
fn fake_device_creation_with_varied_capabilities() {
    // Verify devices with different axis/button counts initialise correctly.
    for (axes, buttons) in [(0, 0), (1, 1), (8, 32), (16, 128)] {
        let dev = FakeDevice::new("Dev", 0x1234, 0x5678, axes, buttons);
        assert_eq!(dev.axes.len(), axes);
        assert_eq!(dev.buttons.len(), buttons);
        assert!(dev.axes.iter().all(|&v| v == 0.0));
        assert!(dev.buttons.iter().all(|&v| !v));
    }
}

#[test]
fn fake_device_axis_injection_full_range() {
    let mut dev = FakeDevice::new("Stick", 0xABCD, 0x0001, 6, 0);

    // Inject values across the full range and verify each independently.
    let values = [-1.0, -0.5, 0.0, 0.25, 0.75, 1.0];
    for (i, &v) in values.iter().enumerate() {
        dev.set_axis(i, v);
    }
    for (i, &expected) in values.iter().enumerate() {
        assert!(
            (dev.axes[i] - expected).abs() < 1e-6,
            "axis {i}: expected {expected}, got {}",
            dev.axes[i]
        );
    }
}

#[test]
fn fake_device_button_injection_patterns() {
    let mut dev = FakeDevice::new("Panel", 0x1111, 0x2222, 0, 8);

    // Press odd-indexed buttons only.
    for i in 0..8 {
        dev.set_button(i, i % 2 == 1);
    }
    for i in 0..8 {
        assert_eq!(dev.buttons[i], i % 2 == 1, "button {i} mismatch");
    }

    // Toggle all buttons off then on.
    for i in 0..8 {
        dev.set_button(i, false);
    }
    assert!(dev.buttons.iter().all(|&b| !b));
    for i in 0..8 {
        dev.set_button(i, true);
    }
    assert!(dev.buttons.iter().all(|&b| b));
}

#[test]
fn fake_device_connect_disconnect_idempotent() {
    let mut dev = FakeDevice::new("Throttle", 0xAAAA, 0xBBBB, 2, 4);
    assert!(!dev.connected);

    // Multiple connects should be fine.
    dev.connect();
    dev.connect();
    assert!(dev.connected);

    // Multiple disconnects should be fine.
    dev.disconnect();
    dev.disconnect();
    assert!(!dev.connected);

    // Re-connect after disconnect.
    dev.connect();
    assert!(dev.connected);
}

#[test]
fn fake_device_multiple_simultaneous() {
    let mut stick = FakeDevice::new("Stick", 0x0001, 0x0001, 3, 2);
    let mut throttle = FakeDevice::new("Throttle", 0x0002, 0x0002, 2, 8);
    let mut rudder = FakeDevice::new("Rudder", 0x0003, 0x0003, 1, 0);

    stick.connect();
    throttle.connect();
    rudder.connect();

    stick.enqueue_input(FakeInput {
        axes: vec![0.5, -0.3, 0.0],
        buttons: vec![true, false],
        delay_ms: 4,
    });
    throttle.enqueue_input(FakeInput {
        axes: vec![0.8, 0.0],
        buttons: vec![false; 8],
        delay_ms: 4,
    });
    rudder.enqueue_input(FakeInput {
        axes: vec![-0.2],
        buttons: vec![],
        delay_ms: 4,
    });

    let s = stick.next_input().unwrap();
    let t = throttle.next_input().unwrap();
    let r = rudder.next_input().unwrap();

    assert!((s.axes[0] - 0.5).abs() < f64::EPSILON);
    assert!((t.axes[0] - 0.8).abs() < f64::EPSILON);
    assert!((r.axes[0] - (-0.2)).abs() < f64::EPSILON);

    // All exhausted.
    assert!(stick.next_input().is_none());
    assert!(throttle.next_input().is_none());
    assert!(rudder.next_input().is_none());
}

#[test]
fn fake_device_custom_capabilities_large_sequence() {
    let mut dev = FakeDevice::new("Custom", 0xFFFF, 0xFFFF, 4, 16);
    dev.connect();

    // Enqueue 100 frames to simulate a long input sequence.
    for i in 0..100 {
        let val = (i as f64 / 99.0) * 2.0 - 1.0; // ramp -1.0 → 1.0
        dev.enqueue_input(FakeInput {
            axes: vec![val, -val, val * 0.5, 0.0],
            buttons: vec![i % 2 == 0; 16],
            delay_ms: 4,
        });
    }

    let mut count = 0;
    while let Some(input) = dev.next_input() {
        assert_eq!(input.axes.len(), 4);
        assert_eq!(input.buttons.len(), 16);
        count += 1;
    }
    assert_eq!(count, 100);

    // After reset, sequence is empty.
    dev.reset();
    assert!(dev.next_input().is_none());
}

// ===========================================================================
// 2. Fake sim backends (6 tests)
// ===========================================================================

#[test]
fn fake_sim_output_recording_ordered() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();

    let commands = [
        "GEAR_DOWN",
        "FLAPS_1",
        "FLAPS_2",
        "SPOILERS_ARM",
        "AUTOPILOT_ON",
    ];
    for cmd in &commands {
        sim.send_command(cmd);
    }

    let recorded = sim.received_commands();
    assert_eq!(recorded.len(), commands.len());
    for (i, &expected) in commands.iter().enumerate() {
        assert_eq!(recorded[i], expected, "command order mismatch at {i}");
    }
}

#[test]
fn fake_sim_expected_output_assertions() {
    let mut sim = FakeSim::new("X-Plane");
    sim.connect();
    sim.send_command("GEAR_TOGGLE");
    sim.send_command("FLAPS_UP");

    // Verify we can assert on specific commands.
    let cmds = sim.received_commands();
    assert!(cmds.contains(&"GEAR_TOGGLE".to_string()));
    assert!(cmds.contains(&"FLAPS_UP".to_string()));
    assert!(!cmds.contains(&"AUTOPILOT_OFF".to_string()));
}

#[test]
fn fake_sim_telemetry_injection_sequence() {
    let mut sim = FakeSim::new("DCS");
    sim.connect();

    // Simulate a takeoff sequence: ground → climb → cruise.
    let phases = [
        (0.0, 0.0, true),
        (1000.0, 150.0, false),
        (5000.0, 200.0, false),
        (10000.0, 250.0, false),
        (35000.0, 450.0, false),
    ];
    for &(alt, spd, ground) in &phases {
        sim.push_snapshot(FakeSnapshot {
            altitude: alt,
            airspeed: spd,
            heading: 90.0,
            pitch: if ground { 0.0 } else { 5.0 },
            roll: 0.0,
            yaw: 0.0,
            on_ground: ground,
        });
    }

    // Replay and verify altitude is monotonically increasing.
    let mut altitudes = Vec::new();
    while let Some(snap) = sim.next_snapshot() {
        altitudes.push(snap.altitude);
    }
    assert_eq!(altitudes.len(), phases.len());
    assert_monotonic(&altitudes);
}

#[test]
fn fake_sim_state_query() {
    let mut sim = FakeSim::new("MSFS");
    assert!(!sim.connected);
    assert!(sim.aircraft.is_none());

    sim.connect();
    sim.set_aircraft("Boeing 737-800");

    assert!(sim.connected);
    assert_eq!(sim.aircraft.as_deref(), Some("Boeing 737-800"));

    // Change aircraft mid-session.
    sim.set_aircraft("Airbus A320neo");
    assert_eq!(sim.aircraft.as_deref(), Some("Airbus A320neo"));
}

#[test]
fn fake_sim_connect_disconnect_preserves_data() {
    let mut sim = FakeSim::new("X-Plane");
    sim.connect();
    sim.send_command("CMD_1");
    sim.push_snapshot(FakeSnapshot {
        altitude: 1000.0,
        airspeed: 120.0,
        heading: 180.0,
        pitch: 0.0,
        roll: 0.0,
        yaw: 0.0,
        on_ground: false,
    });

    // Disconnect shouldn't clear data.
    sim.disconnect();
    assert!(!sim.connected);
    assert_eq!(sim.received_commands().len(), 1);
    assert!(sim.next_snapshot().is_some());

    // Reconnect and continue.
    sim.connect();
    assert!(sim.connected);
    assert!(sim.next_snapshot().is_none()); // already consumed
}

#[test]
fn fake_sim_scenario_playback_full_flight() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.set_aircraft("Cessna 172");

    // Build a full flight scenario.
    let scenario = vec![
        ("ENGINE_START", 0.0, 0.0, true),
        ("TAXI", 0.0, 10.0, true),
        ("TAKEOFF", 500.0, 80.0, false),
        ("CLIMB", 5000.0, 120.0, false),
        ("CRUISE", 8000.0, 130.0, false),
        ("DESCENT", 3000.0, 100.0, false),
        ("APPROACH", 1000.0, 80.0, false),
        ("LANDING", 0.0, 60.0, true),
        ("TAXI_IN", 0.0, 10.0, true),
        ("ENGINE_STOP", 0.0, 0.0, true),
    ];

    for &(cmd, alt, spd, ground) in &scenario {
        sim.send_command(cmd);
        sim.push_snapshot(FakeSnapshot {
            altitude: alt,
            airspeed: spd,
            heading: 270.0,
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            on_ground: ground,
        });
    }

    // Replay and verify.
    assert_eq!(sim.received_commands().len(), scenario.len());
    let mut snap_count = 0;
    while sim.next_snapshot().is_some() {
        snap_count += 1;
    }
    assert_eq!(snap_count, scenario.len());

    // Clear and verify.
    sim.clear();
    assert!(sim.received_commands().is_empty());
    assert!(sim.next_snapshot().is_none());
}

// ===========================================================================
// 3. Deterministic clock (6 tests)
// ===========================================================================

#[test]
fn clock_manual_advance_microseconds() {
    let mut clock = DeterministicClock::new(0);
    assert_eq!(clock.now_us(), 0);

    clock.advance(100);
    assert_eq!(clock.now_us(), 100);

    clock.advance(900);
    assert_eq!(clock.now_us(), 1_000);

    // Large advance.
    clock.advance(1_000_000);
    assert_eq!(clock.now_us(), 1_001_000);
}

#[test]
fn clock_tick_by_tick_stepping() {
    let mut clock = DeterministicClock::new(0);
    let mut timestamps = Vec::new();

    for _ in 0..250 {
        timestamps.push(clock.now_us());
        clock.advance_ticks(1);
    }

    // After 250 ticks at 4000µs each = 1 second.
    assert_eq!(clock.now_us(), 1_000_000);

    // Verify uniform spacing.
    for pair in timestamps.windows(2) {
        assert_eq!(pair[1] - pair[0], TICK_US);
    }
}

#[test]
fn clock_elapsed_time_queries() {
    let start_us = 500_000;
    let mut clock = DeterministicClock::new(start_us);

    // Elapsed from start via difference.
    let t0 = clock.now_us();
    clock.advance_ms(100);
    let t1 = clock.now_us();
    let elapsed = t1 - t0;

    assert_eq!(elapsed, 100_000); // 100ms in µs
    assert_eq!(t1, start_us + 100_000);
}

#[test]
fn clock_timer_scheduling_with_fake_time() {
    let mut clock = DeterministicClock::new(0);

    // Schedule "events" at specific tick intervals.
    let schedule = [10, 25, 50, 100, 250]; // tick offsets
    let mut fired_at = Vec::new();
    let mut next_idx = 0;
    let mut tick = 0u32;

    while next_idx < schedule.len() {
        if tick == schedule[next_idx] {
            fired_at.push(clock.now_us());
            next_idx += 1;
        }
        clock.advance_ticks(1);
        tick += 1;
    }

    // Verify events fired at expected times.
    assert_eq!(fired_at.len(), schedule.len());
    for (i, &tick_offset) in schedule.iter().enumerate() {
        let expected_us = u64::from(tick_offset) * TICK_US;
        assert_eq!(fired_at[i], expected_us, "event {i} fired at wrong time");
    }
}

#[test]
fn clock_reset_restores_zero() {
    let mut clock = DeterministicClock::new(12_345);
    clock.advance_ticks(100);
    assert!(clock.now_us() > 0);

    clock.reset();
    assert_eq!(clock.now_us(), 0);

    // Can advance again from zero.
    clock.advance_ticks(1);
    assert_eq!(clock.now_us(), TICK_US);
}

#[test]
fn clock_mixed_advance_methods() {
    let mut clock = DeterministicClock::new(0);

    clock.advance(500); // 500µs
    clock.advance_ms(2); // +2000µs = 2500µs
    clock.advance_ticks(1); // +4000µs = 6500µs

    assert_eq!(clock.now_us(), 6_500);

    // Advance by many ticks + ms + µs to verify accumulation.
    clock.advance_ticks(250); // +1,000,000µs
    clock.advance_ms(500); // +500,000µs
    clock.advance(42); // +42µs

    assert_eq!(clock.now_us(), 6_500 + 1_000_000 + 500_000 + 42);
}

// ===========================================================================
// 4. Trace replay (5 tests)
// ===========================================================================

fn build_axis_trace(n: usize, interval_us: u64) -> TraceRecording {
    assert!(n >= 2, "build_axis_trace requires n >= 2 to avoid division by zero");
    let mut rec = TraceRecording::new("axis_trace");
    for i in 0..n {
        rec.add_event(TraceEvent {
            timestamp_us: i as u64 * interval_us,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![(i as f64) / (n as f64 - 1.0)],
        });
    }
    rec
}

#[test]
fn trace_record_and_verify_structure() {
    let mut rec = TraceRecording::new("mixed_events");
    rec.device_id = Some("HOTAS-X52".to_owned());

    rec.add_event(TraceEvent {
        timestamp_us: 0,
        event_type: TraceEventType::AxisInput,
        source: TraceSource::Device,
        data: vec![0.0, 0.0, 0.0],
    });
    rec.add_event(TraceEvent {
        timestamp_us: TICK_US,
        event_type: TraceEventType::ButtonPress,
        source: TraceSource::Device,
        data: vec![1.0],
    });
    rec.add_event(TraceEvent {
        timestamp_us: 8_000,
        event_type: TraceEventType::TelemetryUpdate,
        source: TraceSource::Simulator,
        data: vec![1000.0, 120.0, 90.0],
    });
    rec.add_event(TraceEvent {
        timestamp_us: 12_000,
        event_type: TraceEventType::ButtonRelease,
        source: TraceSource::Device,
        data: vec![0.0],
    });

    assert_eq!(rec.event_count(), 4);
    assert_eq!(rec.duration(), 12_000);
    assert_eq!(rec.events_of_type(TraceEventType::AxisInput).len(), 1);
    assert_eq!(rec.events_of_type(TraceEventType::ButtonPress).len(), 1);
    assert_eq!(
        rec.events_of_type(TraceEventType::TelemetryUpdate).len(),
        1
    );
    assert_eq!(
        rec.events_of_type(TraceEventType::ButtonRelease).len(),
        1
    );
    assert_eq!(rec.events_of_type(TraceEventType::SimEvent).len(), 0);
}

#[test]
fn trace_replay_with_callback_collects_all_data() {
    let rec = build_axis_trace(20, TICK_US);
    let mut player = TracePlayer::new(rec);

    let mut values = Vec::new();
    let mut total_delay = 0u64;

    player.with_callback(|evt, delay| {
        values.push(evt.data[0]);
        total_delay += delay;
    });

    assert!(player.is_complete());
    assert_eq!(values.len(), 20);

    // First value should be 0.0, last should be 1.0.
    assert!((values[0] - 0.0).abs() < f64::EPSILON);
    assert!((values[19] - 1.0).abs() < f64::EPSILON);

    // Values should be monotonically increasing.
    assert_monotonic(&values);
}

#[test]
fn trace_format_roundtrip_via_file() {
    let rec = build_axis_trace(50, TICK_US);
    let dir = create_temp_dir("trace-roundtrip");
    let path = dir.path().join("trace.json");

    rec.save_to_file(&path).unwrap();
    let loaded = TraceRecording::load_from_file(&path).unwrap();

    // Structural equivalence.
    assert_eq!(loaded.name, rec.name);
    assert_eq!(loaded.event_count(), rec.event_count());
    assert_eq!(loaded.duration(), rec.duration());

    // Tolerance-based comparison to handle JSON float precision loss.
    let diff = TraceComparator::within_tolerance(1e-12).compare(&rec, &loaded);
    assert!(diff.is_match(), "roundtrip diff: {}", diff.report());
}

#[test]
fn trace_speed_multiplier_affects_delays() {
    let rec = build_axis_trace(10, TICK_US);
    let player = TracePlayer::new(rec);

    // At 1x speed.
    let normal: Vec<_> = player.play_at_speed(1.0).collect();
    // At 4x speed.
    let fast: Vec<_> = player.play_at_speed(4.0).collect();
    // At 0.5x speed.
    let slow: Vec<_> = player.play_at_speed(0.5).collect();

    assert_eq!(normal.len(), fast.len());
    assert_eq!(normal.len(), slow.len());

    // Fast delays should be 1/4 of normal (skip first which is from 0).
    for i in 1..normal.len() {
        assert_eq!(
            fast[i].delay_us,
            normal[i].delay_us / 4,
            "4x speed delay mismatch at {i}"
        );
        assert_eq!(
            slow[i].delay_us,
            normal[i].delay_us * 2,
            "0.5x speed delay mismatch at {i}"
        );
    }
}

#[test]
fn trace_concatenation_via_manual_merge() {
    let mut part1 = TraceRecording::new("part1");
    for i in 0..5 {
        part1.add_event(TraceEvent {
            timestamp_us: i * TICK_US,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![i as f64 * 0.1],
        });
    }

    let mut part2 = TraceRecording::new("part2");
    for i in 0..5 {
        part2.add_event(TraceEvent {
            timestamp_us: i * TICK_US,
            event_type: TraceEventType::AxisInput,
            source: TraceSource::Device,
            data: vec![0.5 + i as f64 * 0.1],
        });
    }

    // Concatenate: offset part2 timestamps by part1's duration.
    let offset = part1.duration() + TICK_US; // gap of one tick
    let mut combined = TraceRecording::new("combined");
    for evt in &part1.events {
        combined.add_event(evt.clone());
    }
    for evt in &part2.events {
        combined.add_event(TraceEvent {
            timestamp_us: evt.timestamp_us + offset,
            ..evt.clone()
        });
    }

    assert_eq!(combined.event_count(), 10);
    // Timestamps should be monotonically non-decreasing.
    let timestamps: Vec<f64> = combined
        .events
        .iter()
        .map(|e| e.timestamp_us as f64)
        .collect();
    assert_monotonic(&timestamps);
}

// ===========================================================================
// 5. Fixture builders (5 tests)
// ===========================================================================

#[test]
fn profile_fixture_builder_multi_simulator() {
    let profiles: Vec<_> = ["MSFS", "X-Plane", "DCS"]
        .iter()
        .map(|&sim| {
            ProfileFixtureBuilder::new(format!("{sim} Profile"))
                .simulator(sim)
                .deadzone(0.05)
                .with_linear_curve()
                .build()
        })
        .collect();

    assert_eq!(profiles.len(), 3);
    for (i, sim) in ["MSFS", "X-Plane", "DCS"].iter().enumerate() {
        assert_eq!(profiles[i].simulator, *sim);
        assert_eq!(profiles[i].deadzone, 0.05);
        assert_eq!(profiles[i].curve_points.len(), 2);
    }
}

#[test]
fn device_fixture_builder_complex_layout() {
    let dev = DeviceFixtureBuilder::new("hotas-x56")
        .name("Logitech X56 HOTAS")
        .with_standard_axes()
        .axis(4, 0.0, "slider_left")
        .axis(5, 0.0, "slider_right")
        .axis(6, 0.0, "rotary_1")
        .with_hotas_buttons()
        .button(4, false, "hat_up")
        .button(5, false, "hat_down")
        .button(6, false, "hat_left")
        .button(7, false, "hat_right")
        .build();

    assert_eq!(dev.id, "hotas-x56");
    assert_eq!(dev.name, "Logitech X56 HOTAS");
    assert_eq!(dev.axes.len(), 7); // 4 standard + 3 custom
    assert_eq!(dev.buttons.len(), 8); // 4 HOTAS + 4 hat
    assert_eq!(dev.axes[4].name, "slider_left");
    assert_eq!(dev.buttons[7].name, "hat_right");
}

#[test]
fn telemetry_fixture_builder_phase_transitions() {
    let on_ramp = TelemetryFixtureBuilder::new().on_ramp().build();
    let climbing = TelemetryFixtureBuilder::new()
        .airspeed(160.0)
        .altitude(3000.0)
        .heading(90.0)
        .vertical_speed(1500.0)
        .on_ground(false)
        .build();
    let cruising = TelemetryFixtureBuilder::new().cruising().build();

    assert!(on_ramp.on_ground);
    assert_eq!(on_ramp.airspeed_kts, 0.0);

    assert!(!climbing.on_ground);
    assert_eq!(climbing.altitude_ft, 3000.0);
    assert_eq!(climbing.vertical_speed_fpm, 1500.0);

    assert!(!cruising.on_ground);
    assert_eq!(cruising.altitude_ft, 35_000.0);
    assert_eq!(cruising.airspeed_kts, 250.0);
}

#[test]
fn composite_fixture_device_and_profile() {
    // Build a device and a matching profile together.
    let dev = DeviceFixtureBuilder::new("warthog")
        .name("TM Warthog")
        .with_standard_axes()
        .with_hotas_buttons()
        .build();

    let profile = ProfileFixtureBuilder::new("warthog-dcs")
        .simulator("DCS")
        .aircraft("A-10C II")
        .deadzone(0.03)
        .curve_point(0.0, 0.0)
        .curve_point(0.5, 0.3)
        .curve_point(1.0, 1.0)
        .build();

    // The device has the axes referenced by the profile's curve.
    assert!(!dev.axes.is_empty());
    assert_eq!(profile.aircraft.as_deref(), Some("A-10C II"));
    assert_eq!(profile.curve_points.len(), 3);
    // Curve should be non-linear (middle point below diagonal).
    assert!(profile.curve_points[1].1 < profile.curve_points[1].0);
}

#[test]
fn fixture_builders_produce_distinct_instances() {
    let dev1 = DeviceFixtureBuilder::new("dev-1")
        .name("Device A")
        .axis(0, 0.5, "x")
        .build();
    let dev2 = DeviceFixtureBuilder::new("dev-2")
        .name("Device B")
        .axis(0, -0.5, "x")
        .build();

    assert_ne!(dev1.id, dev2.id);
    assert_ne!(dev1.name, dev2.name);
    assert_ne!(dev1.axes[0].value, dev2.axes[0].value);

    let p1 = ProfileFixtureBuilder::new("p1").deadzone(0.01).build();
    let p2 = ProfileFixtureBuilder::new("p2").deadzone(0.10).build();
    assert_ne!(p1.name, p2.name);
    assert_ne!(p1.deadzone, p2.deadzone);
}

// ===========================================================================
// 6. Assertion helpers (5 tests)
// ===========================================================================

#[test]
fn axis_value_assertions_with_tolerance() {
    // In-range checks across the full axis domain.
    assert_axis_in_range(0.0, -1.0, 1.0, "pitch_centre");
    assert_axis_in_range(-1.0, -1.0, 1.0, "roll_min");
    assert_axis_in_range(1.0, -1.0, 1.0, "yaw_max");

    // Approximate equality for floating-point axis values.
    assert_approx_eq(0.500_000_1, 0.5, 1e-6);
    assert_approx_eq(-0.999_999_9, -1.0, 1e-6);

    // Deadzone symmetry: small inputs should map to near-zero outputs.
    let deadzone_pairs = vec![
        (0.01, 0.0),
        (-0.01, 0.0),
        (0.04, 0.0),
        (-0.04, 0.0),
        (0.5, 0.48),
        (-0.5, -0.48),
    ];
    assert_symmetric_deadzone(&deadzone_pairs, 0.05);
}

#[test]
fn timing_assertions_comprehensive() {
    // Latency under threshold.
    assert_latency_under(200, 300, "hid_write");
    assert_latency_under(0, 300, "zero_latency");
    assert_latency_under(300, 300, "exact_threshold");

    // Jitter within budget: inter-sample intervals around 4000µs with slight
    // variation.
    let intervals: Vec<u64> = (0..100)
        .map(|i| {
            let jitter: u64 = if i % 3 == 0 { 10 } else { 0 };
            TICK_US + jitter
        })
        .collect();
    assert_jitter_under(&intervals, 20);

    // Frequency check for 250Hz samples.
    let perfect_250hz: Vec<u64> = (0..100).map(|i| i * TICK_US).collect();
    assert_frequency_within(&perfect_250hz, 250.0, 0.1);
}

#[test]
fn event_sequence_assertions_monotonic_and_bounded() {
    // Monotonically increasing axis values.
    let ramp: Vec<f64> = (0..50).map(|i| i as f64 / 49.0).collect();
    assert_monotonic(&ramp);

    // Bounded rate of change (smooth input).
    let smooth: Vec<f64> = (0..100).map(|i| (i as f64 * 0.01).sin()).collect();
    assert_bounded_rate(&smooth, 0.02); // sin changes slowly

    // In-range for all values.
    for &v in &smooth {
        assert_in_range(v, -1.0, 1.0);
    }
}

#[test]
fn data_quality_assertions_nan_inf() {
    // Clean data passes.
    let clean: Vec<f64> = (0..100).map(|i| i as f64 * 0.01).collect();
    assert_no_nan(&clean, "clean_axes");
    assert_no_inf(&clean, "clean_axes");

    // Verify edge values are okay.
    assert_no_nan(&[0.0, -0.0, f64::MIN_POSITIVE, f64::MAX], "edge_values");
    assert_no_inf(&[0.0, 1e10, -1e10, f64::MIN_POSITIVE], "large_values");
}

#[test]
#[should_panic(expected = "NaN")]
fn data_quality_assertion_catches_nan() {
    let bad = vec![0.0, 0.5, f64::NAN, 0.7];
    assert_no_nan(&bad, "axis_data");
}

// ===========================================================================
// Additional depth tests (beyond the 6 core areas)
// ===========================================================================

#[test]
fn snapshot_store_large_batch() {
    let mut store = SnapshotStore::new();

    // Record 100 snapshots.
    for i in 0..100 {
        store.record(format!("snap_{i}"), format!("content_{i}"));
    }

    assert_eq!(store.all_names().len(), 100);

    // Verify all match.
    for i in 0..100 {
        assert_eq!(
            store.verify(&format!("snap_{i}"), &format!("content_{i}")),
            SnapshotResult::Match
        );
    }

    // Detect a mismatch.
    assert!(matches!(
        store.verify("snap_0", "wrong"),
        SnapshotResult::Mismatch { .. }
    ));
}

#[test]
fn test_config_and_harness_integration() {
    let config = TestConfigBuilder::default()
        .with_timeout(Duration::from_secs(5))
        .with_poll_interval(Duration::from_millis(5))
        .build();

    let harness = TestHarness::new(config);
    assert!(!harness.timed_out());
    assert_eq!(harness.poll_interval(), Duration::from_millis(5));
}

#[test]
fn test_device_builder_multiple_instances() {
    let devices: Vec<_> = (0..5)
        .map(|i| {
            TestDeviceBuilder::default()
                .with_vid_pid(0x0600 + i, 0x0700 + i)
                .with_serial(format!("SN-{i:04}"))
                .with_path(format!("test://device/{i}"))
                .build()
        })
        .collect();

    // All should have unique VID/PID combinations.
    for (i, dev) in devices.iter().enumerate() {
        assert_eq!(dev.vendor_id, 0x0600 + i as u16);
        assert_eq!(dev.product_id, 0x0700 + i as u16);
    }
}

#[test]
fn wait_for_condition_immediate_true() {
    let result =
        wait_for_condition(Duration::from_millis(100), Duration::from_millis(5), || true);
    assert!(result);
}

#[test]
fn trace_comparator_tolerance_boundary() {
    let mut a = TraceRecording::new("a");
    a.add_event(TraceEvent {
        timestamp_us: 0,
        event_type: TraceEventType::AxisInput,
        source: TraceSource::Device,
        data: vec![1.0],
    });

    let mut b = TraceRecording::new("b");
    b.add_event(TraceEvent {
        timestamp_us: 0,
        event_type: TraceEventType::AxisInput,
        source: TraceSource::Device,
        data: vec![1.005],
    });

    // Tolerance of 0.005 should match exactly at boundary.
    let diff = TraceComparator::within_tolerance(0.005).compare(&a, &b);
    assert!(diff.is_match());

    // Tolerance of 0.004 should fail.
    let diff = TraceComparator::within_tolerance(0.004).compare(&a, &b);
    assert!(!diff.is_match());
}

#[test]
fn trace_player_seek_and_advance_interleaved() {
    let rec = build_axis_trace(20, TICK_US);
    let mut player = TracePlayer::new(rec);

    // Seek to middle.
    player.seek_to(40_000); // event at index 10
    assert_eq!(player.remaining_events(), 10);

    // Advance a bit.
    let events = player.advance_to(52_000); // up to event at 52_000µs
    assert!(!events.is_empty());

    // Seek back to start (reset effectively).
    player.reset();
    assert_eq!(player.remaining_events(), 20);
}

#[test]
fn clock_drives_fake_device_timing() {
    let mut clock = DeterministicClock::new(0);
    let mut dev = FakeDevice::new("Timed Stick", 0x1234, 0x5678, 2, 0);
    dev.connect();

    // Enqueue inputs at 250Hz rate.
    for i in 0..10 {
        dev.enqueue_input(FakeInput {
            axes: vec![i as f64 * 0.1, 0.0],
            buttons: vec![],
            delay_ms: 4,
        });
    }

    let mut timeline = Vec::new();
    while let Some(input) = dev.next_input() {
        timeline.push((clock.now_us(), input.axes[0]));
        clock.advance_ms(input.delay_ms);
    }

    assert_eq!(timeline.len(), 10);
    // Verify timestamps are uniformly spaced at 4ms.
    for pair in timeline.windows(2) {
        assert_eq!(pair[1].0 - pair[0].0, TICK_US);
    }
}
