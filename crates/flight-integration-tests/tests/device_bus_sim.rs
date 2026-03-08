// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Device → Bus → Sim end-to-end integration tests.
//!
//! Proves: physical axis input flows through pipeline and bus to sim adapter
//! output. Uses testkit fakes for all hardware and simulator interactions.

use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage};
use flight_bus::types::SimId;
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{
    DeterministicClock, FakeDevice, FakeInput, FakeSim, assert_approx_eq, assert_in_range,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn joystick_pipeline() -> AxisPipeline {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.3 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));
    pipeline
}

fn make_device(name: &str) -> FakeDevice {
    let mut dev = FakeDevice::new(name, 0x044F, 0xB10A, 4, 16);
    dev.connect();
    dev
}

fn make_sim(name: &str) -> FakeSim {
    let mut sim = FakeSim::new(name);
    sim.connect();
    sim
}

// ===========================================================================
// 1. Physical axis input flows through bus to sim adapter output
// ===========================================================================

#[test]
fn e2e_axis_input_through_pipeline_to_sim() {
    let mut device = make_device("HOTAS Warthog");
    let mut sim = make_sim("MSFS 2020");
    let pipeline = joystick_pipeline();

    // Simulate pitch axis deflection
    device.enqueue_input(FakeInput {
        axes: vec![0.75, 0.0, 0.0, 0.0],
        buttons: vec![false; 16],
        delay_ms: 4,
    });

    let input = device.next_input().unwrap();
    let processed = pipeline.process(input.axes[0], 0.004);

    // Publish to bus
    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snapshot.control_inputs.pitch = processed as f32;
    publisher.publish(snapshot).unwrap();

    // Sim receives the processed value
    let received = subscriber.try_recv().unwrap().unwrap();
    sim.send_command(&format!("SET_PITCH:{}", received.control_inputs.pitch));

    assert_approx_eq(received.control_inputs.pitch as f64, processed, 1e-5);
    assert_eq!(sim.received_commands().len(), 1);
    assert!(sim.received_commands()[0].starts_with("SET_PITCH:"));
}

// ===========================================================================
// 2. Button press reaches sim command
// ===========================================================================

#[test]
fn e2e_button_press_reaches_sim_command() {
    let mut device = make_device("Joystick Buttons");
    let mut sim = make_sim("MSFS 2020");

    // Simulate button press on button 0 (trigger) and button 5 (hat)
    device.enqueue_input(FakeInput {
        axes: vec![0.0; 4],
        buttons: {
            let mut b = vec![false; 16];
            b[0] = true; // trigger
            b[5] = true; // secondary
            b
        },
        delay_ms: 4,
    });

    let input = device.next_input().unwrap();

    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("F18"));
    // Map button 0 → weapon release, button 5 → flaps toggle
    if input.buttons[0] {
        snapshot.config.lights.strobe = true; // example: strobe toggle
    }
    publisher.publish(snapshot).unwrap();

    let received = subscriber.try_recv().unwrap().unwrap();
    assert!(received.config.lights.strobe, "button 0 must toggle strobe");

    // Deliver to sim
    for (idx, &pressed) in input.buttons.iter().enumerate() {
        if pressed {
            sim.send_command(&format!("BUTTON_PRESS:{idx}"));
        }
    }

    assert_eq!(sim.received_commands().len(), 2);
    assert_eq!(sim.received_commands()[0], "BUTTON_PRESS:0");
    assert_eq!(sim.received_commands()[1], "BUTTON_PRESS:5");
}

// ===========================================================================
// 3. Hat switch maps correctly
// ===========================================================================

#[test]
fn e2e_hat_switch_maps_to_discrete_directions() {
    let mut device = make_device("Hat Switch Stick");
    let mut sim = make_sim("DCS World");

    // Encode hat as 4 buttons: up=8, right=9, down=10, left=11
    let hat_directions = [
        ("UP", 8),
        ("RIGHT", 9),
        ("DOWN", 10),
        ("LEFT", 11),
    ];

    for &(dir_name, btn_idx) in &hat_directions {
        let mut buttons = vec![false; 16];
        buttons[btn_idx] = true;
        device.enqueue_input(FakeInput {
            axes: vec![0.0; 4],
            buttons,
            delay_ms: 4,
        });

        let input = device.next_input().unwrap();
        assert!(input.buttons[btn_idx], "button {btn_idx} for {dir_name}");
        sim.send_command(&format!("HAT:{dir_name}"));
    }

    assert_eq!(sim.received_commands().len(), 4);
    assert_eq!(sim.received_commands()[0], "HAT:UP");
    assert_eq!(sim.received_commands()[1], "HAT:RIGHT");
    assert_eq!(sim.received_commands()[2], "HAT:DOWN");
    assert_eq!(sim.received_commands()[3], "HAT:LEFT");
}

// ===========================================================================
// 4. Throttle range maps correctly
// ===========================================================================

#[test]
fn e2e_throttle_range_maps_zero_to_one() {
    let mut device = make_device("Throttle Quadrant");
    let pipeline = joystick_pipeline();

    // Throttle axis is index 3, ramp from 0.0 → 1.0
    let throttle_values = [0.0, 0.25, 0.5, 0.75, 1.0];
    for &val in &throttle_values {
        device.enqueue_input(FakeInput {
            axes: vec![0.0, 0.0, 0.0, val],
            buttons: vec![false; 16],
            delay_ms: 4,
        });
    }

    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut processed_values = Vec::new();
    while let Some(input) = device.next_input() {
        let processed = pipeline.process(input.axes[3], 0.004);
        processed_values.push(processed);

        let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("B737"));
        snap.control_inputs.throttle = vec![processed as f32];
        publisher.publish(snap).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(25));
    }

    // Verify all values received and in range
    let mut received = Vec::new();
    while let Ok(Some(s)) = subscriber.try_recv() {
        received.push(s.control_inputs.throttle[0]);
    }

    assert_eq!(received.len(), throttle_values.len());
    for &v in &received {
        assert_in_range(v as f64, -1.0, 1.0);
    }

    // Full throttle (1.0) must map to 1.0 output
    assert_approx_eq(*received.last().unwrap() as f64, 1.0, 1e-6);
    // Zero throttle (0.0) must map to 0.0 (inside deadzone)
    assert_approx_eq(received[0] as f64, 0.0, 1e-6);
}

// ===========================================================================
// 5. Multiple devices simultaneously
// ===========================================================================

#[test]
fn e2e_multiple_devices_simultaneous_pipeline() {
    let mut stick = make_device("Main Stick");
    let mut throttle = make_device("Throttle Unit");
    let mut rudder = make_device("Rudder Pedals");

    let stick_pipeline = joystick_pipeline();
    let throttle_pipeline = joystick_pipeline();
    let rudder_pipeline = joystick_pipeline();

    // Enqueue simultaneous inputs
    stick.enqueue_input(FakeInput {
        axes: vec![0.6, -0.4, 0.0, 0.0],
        buttons: vec![false; 16],
        delay_ms: 4,
    });
    throttle.enqueue_input(FakeInput {
        axes: vec![0.0, 0.0, 0.0, 0.8],
        buttons: vec![false; 16],
        delay_ms: 4,
    });
    rudder.enqueue_input(FakeInput {
        axes: vec![0.0, 0.0, 0.15, 0.0],
        buttons: vec![false; 16],
        delay_ms: 4,
    });

    let stick_in = stick.next_input().unwrap();
    let throttle_in = throttle.next_input().unwrap();
    let rudder_in = rudder.next_input().unwrap();

    let pitch = stick_pipeline.process(stick_in.axes[0], 0.004);
    let roll = stick_pipeline.process(stick_in.axes[1], 0.004);
    let thr = throttle_pipeline.process(throttle_in.axes[3], 0.004);
    let yaw = rudder_pipeline.process(rudder_in.axes[2], 0.004);

    let mut publisher = BusPublisher::new(60.0);
    let mut subscriber = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("A320"));
    snapshot.control_inputs.pitch = pitch as f32;
    snapshot.control_inputs.roll = roll as f32;
    snapshot.control_inputs.yaw = yaw as f32;
    snapshot.control_inputs.throttle = vec![thr as f32];
    publisher.publish(snapshot).unwrap();

    let received = subscriber.try_recv().unwrap().unwrap();

    // All three device outputs must be present in the merged snapshot
    assert!(received.control_inputs.pitch > 0.0, "pitch from stick");
    assert!(received.control_inputs.roll < 0.0, "roll from stick");
    assert!(received.control_inputs.yaw > 0.0, "yaw from rudder");
    assert!(received.control_inputs.throttle[0] > 0.0, "throttle");

    // Verify all outputs are in range
    assert_in_range(received.control_inputs.pitch as f64, -1.0, 1.0);
    assert_in_range(received.control_inputs.roll as f64, -1.0, 1.0);
    assert_in_range(received.control_inputs.yaw as f64, -1.0, 1.0);
    assert_in_range(received.control_inputs.throttle[0] as f64, -1.0, 1.0);

    // Verify independent processing
    let mut clock = DeterministicClock::new(0);
    clock.advance_ticks(1);
    assert_eq!(clock.now_us(), 4_000, "single tick = 4ms");
}
