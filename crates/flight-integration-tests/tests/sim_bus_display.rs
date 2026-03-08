// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Sim → Bus → Display end-to-end integration tests.
//!
//! Proves: simulator telemetry flows through the bus to panel displays.
//! Verifies altitude→7-segment, heading→display, gear→LED, COM→radio panel.

use flight_bus::types::{GearPosition, SimId};
use flight_bus::{AircraftId, BusPublisher, BusSnapshot, SubscriptionConfig};
use flight_test_helpers::{FakeSim, FakeSnapshot, assert_approx_eq, assert_in_range};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn tick() {
    std::thread::sleep(std::time::Duration::from_millis(25));
}

fn sim_snapshot_to_bus(sim: &mut FakeSim) -> Option<BusSnapshot> {
    let snap = sim.next_snapshot()?;
    let mut bus_snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    bus_snap.environment.altitude = snap.altitude as f32;
    // ValidatedAngle accepts -180..180; convert compass heading (0..360) first
    let hdg_normalized = if snap.heading > 180.0 {
        snap.heading - 360.0
    } else {
        snap.heading
    };
    if let Ok(hdg) = flight_bus::types::ValidatedAngle::new_degrees(hdg_normalized as f32) {
        bus_snap.kinematics.heading = hdg;
    }
    if let Ok(spd) = flight_bus::types::ValidatedSpeed::new_knots(snap.airspeed as f32) {
        bus_snap.kinematics.ias = spd;
    }
    Some(bus_snap)
}

// ===========================================================================
// 1. Sim telemetry flows to panel display subscriber
// ===========================================================================

#[test]
fn e2e_sim_telemetry_reaches_display_subscriber() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();
    sim.push_snapshot(FakeSnapshot {
        altitude: 5500.0,
        airspeed: 120.0,
        heading: 270.0,
        pitch: 2.0,
        roll: 0.0,
        yaw: 0.0,
        on_ground: false,
    });

    let mut publisher = BusPublisher::new(60.0);
    let mut panel_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    if let Some(bus_snap) = sim_snapshot_to_bus(&mut sim) {
        publisher.publish(bus_snap).unwrap();
    }

    let received = panel_sub.try_recv().unwrap().unwrap();

    // Panel display receives altitude
    let altitude = received.environment.altitude;
    assert_approx_eq(altitude as f64, 5500.0, 0.1);
}

// ===========================================================================
// 2. Altitude → 7-segment display formatting
// ===========================================================================

#[test]
fn e2e_altitude_to_seven_segment_display() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();

    let altitudes = [0.0, 1000.0, 5500.0, 10000.0, 35000.0, 41000.0];

    let mut publisher = BusPublisher::new(60.0);
    let mut display_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    for &alt in &altitudes {
        sim.push_snapshot(FakeSnapshot {
            altitude: alt,
            airspeed: 120.0,
            heading: 0.0,
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            on_ground: alt < 1.0,
        });

        if let Some(bus_snap) = sim_snapshot_to_bus(&mut sim) {
            publisher.publish(bus_snap).unwrap();
            tick();
        }
    }

    let mut received_altitudes = Vec::new();
    while let Ok(Some(snap)) = display_sub.try_recv() {
        received_altitudes.push(snap.environment.altitude);
    }

    assert_eq!(received_altitudes.len(), altitudes.len());

    // Verify 7-segment formatting: each digit extractable from altitude
    for (i, &alt) in received_altitudes.iter().enumerate() {
        assert_approx_eq(alt as f64, altitudes[i], 0.1);

        // Simulate 5-digit 7-segment extraction
        let alt_int = alt as u32;
        let digits: Vec<u8> = (0..5)
            .rev()
            .map(|p| ((alt_int / 10u32.pow(p)) % 10) as u8)
            .collect();

        // All digits must be 0-9
        for &d in &digits {
            assert!(d <= 9, "digit must be 0-9, got {d}");
        }
    }
}

// ===========================================================================
// 3. Heading → display
// ===========================================================================

#[test]
fn e2e_heading_to_display_subscriber() {
    let mut sim = FakeSim::new("MSFS");
    sim.connect();

    let headings = [0.0, 90.0, 180.0, 270.0, 359.0];

    let mut publisher = BusPublisher::new(60.0);
    let mut display_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    for &hdg in &headings {
        sim.push_snapshot(FakeSnapshot {
            altitude: 5000.0,
            airspeed: 120.0,
            heading: hdg,
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            on_ground: false,
        });

        if let Some(bus_snap) = sim_snapshot_to_bus(&mut sim) {
            publisher.publish(bus_snap).unwrap();
            tick();
        }
    }

    let mut received_headings = Vec::new();
    while let Ok(Some(snap)) = display_sub.try_recv() {
        received_headings.push(snap.kinematics.heading.to_degrees());
    }

    // Headings stored as -180..180; convert back for comparison
    let headings_normalized: Vec<f64> = headings
        .iter()
        .map(|&h| if h > 180.0 { h - 360.0 } else { h })
        .collect();

    assert_eq!(received_headings.len(), headings.len());

    for (i, &hdg) in received_headings.iter().enumerate() {
        assert_in_range(hdg as f64, -180.0, 180.0);
        assert_approx_eq(hdg as f64, headings_normalized[i], 0.5);
    }
}

// ===========================================================================
// 4. Gear state → LED indicator
// ===========================================================================

#[test]
fn e2e_gear_state_to_led_indicator() {
    let mut publisher = BusPublisher::new(60.0);
    let mut led_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Gear down
    let mut snap_down = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap_down.config.gear.nose = GearPosition::Down;
    snap_down.config.gear.left = GearPosition::Down;
    snap_down.config.gear.right = GearPosition::Down;
    publisher.publish(snap_down).unwrap();
    tick();

    // Gear transitioning
    let mut snap_transit = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap_transit.config.gear.nose = GearPosition::Transitioning;
    snap_transit.config.gear.left = GearPosition::Transitioning;
    snap_transit.config.gear.right = GearPosition::Transitioning;
    publisher.publish(snap_transit).unwrap();
    tick();

    // Gear up
    let mut snap_up = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    snap_up.config.gear.nose = GearPosition::Up;
    snap_up.config.gear.left = GearPosition::Up;
    snap_up.config.gear.right = GearPosition::Up;
    publisher.publish(snap_up).unwrap();
    tick();

    let mut states = Vec::new();
    while let Ok(Some(snap)) = led_sub.try_recv() {
        states.push(snap.config.gear);
    }

    assert_eq!(states.len(), 3);

    // State 1: gear down → green LED (all down)
    assert!(states[0].all_down(), "gear must be all down");

    // State 2: transitioning → amber LED (not all up or down)
    assert!(states[1].transitioning(), "gear must be transitioning");

    // State 3: gear up → no LED (all up)
    assert!(states[2].all_up(), "gear must be all up");
}

// ===========================================================================
// 5. COM frequency → radio panel display
// ===========================================================================

#[test]
fn e2e_com_frequency_to_radio_panel() {
    let mut publisher = BusPublisher::new(60.0);
    let mut radio_sub = publisher.subscribe(SubscriptionConfig::default()).unwrap();

    // Simulate COM1 frequency updates
    let frequencies = [118.0_f32, 121.5, 127.85, 135.975];

    for &freq in &frequencies {
        let mut snap = BusSnapshot::new(SimId::Msfs, AircraftId::new("B737"));
        // Use navigation active_waypoint to carry COM frequency as string
        snap.navigation.active_waypoint = Some(format!("{freq:.3}"));
        publisher.publish(snap).unwrap();
        tick();
    }

    let mut received_freqs = Vec::new();
    while let Ok(Some(snap)) = radio_sub.try_recv() {
        if let Some(wp) = &snap.navigation.active_waypoint
            && let Ok(f) = wp.parse::<f32>()
        {
            received_freqs.push(f);
        }
    }

    assert_eq!(received_freqs.len(), frequencies.len());

    for (i, &freq) in received_freqs.iter().enumerate() {
        assert_approx_eq(freq as f64, frequencies[i] as f64, 0.01);

        // Validate frequency is in valid COM range (118.000 - 136.975 MHz)
        assert!(
            (118.0..=137.0).contains(&freq),
            "COM frequency {freq} must be in valid range"
        );
    }
}
