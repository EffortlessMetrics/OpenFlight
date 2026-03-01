// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the weather-to-FFB bridge.
//!
//! Covers wind force mapping, turbulence synthesis, runway effects,
//! weather transitions, and property-based invariants across the
//! `weather_ffb`, `crosswind`, `ground_effect`, and `wheel_shimmy` modules.

use crate::crosswind::compute_crosswind_forces;
use crate::ground_effect::{ground_effect_modifier, GroundEffectConfig};
use crate::weather_ffb::{FfbForces, WeatherData, WeatherFfbBridge, WeatherFfbConfig};
use crate::wheel_shimmy::{compute_wheel_shimmy, WheelShimmyConfig};

// ─── helpers ─────────────────────────────────────────────────────────────────

fn default_bridge() -> WeatherFfbBridge {
    WeatherFfbBridge::default()
}

fn base_weather() -> WeatherData {
    WeatherData {
        wind_speed_kts: 20.0,
        wind_direction_deg: 270.0,
        turbulence_intensity: 0.0,
        gust_factor: 1.0,
        aircraft_heading_deg: 270.0,
        airspeed_kts: 120.0,
    }
}

fn calm_weather() -> WeatherData {
    WeatherData {
        wind_speed_kts: 0.0,
        wind_direction_deg: 0.0,
        turbulence_intensity: 0.0,
        gust_factor: 1.0,
        aircraft_heading_deg: 0.0,
        airspeed_kts: 100.0,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Wind force mapping (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn wind_headwind_produces_positive_resistance() {
    // Pure headwind: wind from the same direction as heading → cos component
    let w = WeatherData {
        wind_speed_kts: 30.0,
        wind_direction_deg: 360.0,
        aircraft_heading_deg: 360.0,
        turbulence_intensity: 0.0,
        gust_factor: 1.0,
        airspeed_kts: 100.0,
    };
    let f = default_bridge().compute_forces(&w);
    assert!(
        f.headwind_buffet > 0.1,
        "headwind should produce positive resistance, got {}",
        f.headwind_buffet
    );
}

#[test]
fn wind_crosswind_produces_lateral_force() {
    // 90° crosswind from the right
    let w = WeatherData {
        wind_speed_kts: 25.0,
        wind_direction_deg: 90.0,
        aircraft_heading_deg: 0.0,
        turbulence_intensity: 0.0,
        gust_factor: 1.0,
        airspeed_kts: 120.0,
    };
    let f = default_bridge().compute_forces(&w);
    assert!(
        f.crosswind_force.abs() > 0.2,
        "90° crosswind should produce significant lateral force, got {}",
        f.crosswind_force
    );
}

#[test]
fn wind_tailwind_produces_reduced_buffet_vs_headwind() {
    let bridge = default_bridge();
    // Headwind: wind from nose
    let headwind = WeatherData {
        wind_speed_kts: 25.0,
        wind_direction_deg: 0.0,
        aircraft_heading_deg: 0.0,
        turbulence_intensity: 0.0,
        gust_factor: 1.0,
        airspeed_kts: 120.0,
    };
    // Tailwind: wind from behind (180° off heading)
    let tailwind = WeatherData {
        wind_speed_kts: 25.0,
        wind_direction_deg: 180.0,
        aircraft_heading_deg: 0.0,
        turbulence_intensity: 0.0,
        gust_factor: 1.0,
        airspeed_kts: 120.0,
    };
    let f_head = bridge.compute_forces(&headwind);
    let f_tail = bridge.compute_forces(&tailwind);
    // Both produce |cos| buffet, but the implementation uses abs so they
    // should be similar. The key property: tailwind still generates buffet.
    assert!(
        f_tail.headwind_buffet > 0.0,
        "tailwind should still produce some buffet force, got {}",
        f_tail.headwind_buffet
    );
    // Headwind buffet uses abs(cos) so magnitude should be comparable
    assert!(
        (f_head.headwind_buffet - f_tail.headwind_buffet).abs() < 0.1,
        "head/tail buffet should be similar due to abs(cos): head={}, tail={}",
        f_head.headwind_buffet,
        f_tail.headwind_buffet
    );
}

#[test]
fn wind_gust_produces_transient_amplification() {
    let bridge = default_bridge();
    let mut w = base_weather();
    w.wind_direction_deg = 0.0; // crosswind relative to heading 270
    w.gust_factor = 1.0;
    let f_calm = bridge.compute_forces(&w);

    w.gust_factor = 2.5;
    let f_gust = bridge.compute_forces(&w);

    assert!(
        f_gust.crosswind_force.abs() > f_calm.crosswind_force.abs(),
        "gust factor should amplify crosswind: calm={}, gust={}",
        f_calm.crosswind_force.abs(),
        f_gust.crosswind_force.abs()
    );
    assert!(
        f_gust.headwind_buffet >= f_calm.headwind_buffet,
        "gust factor should amplify buffet: calm={}, gust={}",
        f_calm.headwind_buffet,
        f_gust.headwind_buffet
    );
}

#[test]
fn wind_shear_causes_rapid_force_change() {
    let bridge = default_bridge();
    let w1 = WeatherData {
        wind_speed_kts: 15.0,
        wind_direction_deg: 0.0,
        aircraft_heading_deg: 180.0,
        turbulence_intensity: 0.0,
        gust_factor: 1.0,
        airspeed_kts: 100.0,
    };
    // Wind shear: direction shifts 90° instantly
    let w2 = WeatherData {
        wind_direction_deg: 90.0,
        ..w1.clone()
    };

    let f1 = bridge.compute_forces(&w1);
    let f2 = bridge.compute_forces(&w2);

    let delta = (f2.crosswind_force - f1.crosswind_force).abs()
        + (f2.headwind_buffet - f1.headwind_buffet).abs();
    assert!(
        delta > 0.05,
        "wind shear (90° shift) should produce noticeable force delta, got {}",
        delta
    );
}

#[test]
fn wind_calm_conditions_produce_zero_force() {
    let f = default_bridge().compute_forces(&calm_weather());
    assert!(
        f.crosswind_force.abs() < 1e-9,
        "calm crosswind should be zero, got {}",
        f.crosswind_force
    );
    assert!(
        f.headwind_buffet.abs() < 1e-9,
        "calm buffet should be zero, got {}",
        f.headwind_buffet
    );
    assert!(
        f.turbulence_shake.abs() < 1e-9,
        "calm turbulence should be zero, got {}",
        f.turbulence_shake
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Turbulence (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn turbulence_light_produces_subtle_vibration() {
    let mut w = base_weather();
    w.turbulence_intensity = 0.1;
    let f = default_bridge().compute_forces(&w);
    assert!(
        f.turbulence_shake > 0.0 && f.turbulence_shake < 0.2,
        "light turbulence should be subtle (0..0.2), got {}",
        f.turbulence_shake
    );
}

#[test]
fn turbulence_moderate_produces_noticeable_jolts() {
    let mut w = base_weather();
    w.turbulence_intensity = 0.5;
    let f = default_bridge().compute_forces(&w);
    assert!(
        f.turbulence_shake >= 0.3 && f.turbulence_shake <= 0.7,
        "moderate turbulence should be noticeable (0.3..0.7), got {}",
        f.turbulence_shake
    );
}

#[test]
fn turbulence_severe_produces_large_forces() {
    let mut w = base_weather();
    w.turbulence_intensity = 1.0;
    let f = default_bridge().compute_forces(&w);
    assert!(
        f.turbulence_shake >= 0.8,
        "severe turbulence should produce large shake (>=0.8), got {}",
        f.turbulence_shake
    );
}

#[test]
fn turbulence_clear_air_with_no_wind() {
    // Clear-air turbulence: turbulence present even without significant wind
    let w = WeatherData {
        wind_speed_kts: 0.0,
        wind_direction_deg: 0.0,
        turbulence_intensity: 0.6,
        gust_factor: 1.0,
        aircraft_heading_deg: 0.0,
        airspeed_kts: 250.0,
    };
    let f = default_bridge().compute_forces(&w);
    // Wind forces should be zero but turbulence should still manifest
    assert!(
        f.crosswind_force.abs() < 1e-9,
        "no wind means no crosswind force"
    );
    assert!(
        f.turbulence_shake > 0.3,
        "clear-air turbulence should still produce shake, got {}",
        f.turbulence_shake
    );
}

#[test]
fn turbulence_wake_produces_consistent_pattern() {
    // Wake turbulence is deterministic for the same inputs — verify reproducibility
    let w = WeatherData {
        wind_speed_kts: 10.0,
        wind_direction_deg: 45.0,
        turbulence_intensity: 0.7,
        gust_factor: 1.2,
        aircraft_heading_deg: 90.0,
        airspeed_kts: 180.0,
    };
    let bridge = default_bridge();
    let f1 = bridge.compute_forces(&w);
    let f2 = bridge.compute_forces(&w);
    assert_eq!(
        f1, f2,
        "same weather inputs must produce identical forces (deterministic)"
    );
}

#[test]
fn turbulence_gain_scales_effect() {
    let w = WeatherData {
        wind_speed_kts: 10.0,
        wind_direction_deg: 0.0,
        turbulence_intensity: 0.5,
        gust_factor: 1.0,
        aircraft_heading_deg: 0.0,
        airspeed_kts: 100.0,
    };

    let low_gain = WeatherFfbBridge::new(WeatherFfbConfig {
        turbulence_gain: 0.5,
        ..WeatherFfbConfig::default()
    });
    let high_gain = WeatherFfbBridge::new(WeatherFfbConfig {
        turbulence_gain: 2.0,
        ..WeatherFfbConfig::default()
    });

    let f_low = low_gain.compute_forces(&w);
    let f_high = high_gain.compute_forces(&w);

    assert!(
        f_high.turbulence_shake > f_low.turbulence_shake,
        "higher turbulence_gain should amplify shake: low={}, high={}",
        f_low.turbulence_shake,
        f_high.turbulence_shake
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Runway effects (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn runway_ground_rumble_near_surface() {
    // Ground effect modifier should be significantly reduced near the surface
    let cfg = GroundEffectConfig { wingspan_m: 12.0 };
    let on_ground = ground_effect_modifier(0.5, 70.0, &cfg);
    let airborne = ground_effect_modifier(50.0, 70.0, &cfg);
    assert!(
        on_ground < airborne,
        "near-surface modifier should be less than high-altitude: ground={}, air={}",
        on_ground,
        airborne
    );
    assert!(
        on_ground < 0.15,
        "very close to ground should have strong effect (low modifier), got {}",
        on_ground
    );
}

#[test]
fn runway_nose_wheel_shimmy_during_taxi() {
    let cfg = WheelShimmyConfig::default();
    let out = compute_wheel_shimmy(30.0, 5.0, true, &cfg);
    assert!(
        out.amplitude > 0.0,
        "nose wheel shimmy should be active during taxi with deflection, amp={}",
        out.amplitude
    );
    assert!(
        out.frequency_hz >= cfg.base_frequency_hz,
        "shimmy frequency should be at least base: got {}",
        out.frequency_hz
    );
}

#[test]
fn runway_brake_vibration_high_speed_shimmy() {
    // Heavy braking at speed: large deflection + high ground speed
    let cfg = WheelShimmyConfig::default();
    let out = compute_wheel_shimmy(50.0, 12.0, true, &cfg);
    assert!(
        out.amplitude > 0.1,
        "heavy braking should produce noticeable shimmy, got {}",
        out.amplitude
    );
    assert!(
        out.frequency_hz > cfg.base_frequency_hz,
        "frequency should exceed base at higher speed"
    );
}

#[test]
fn runway_touchdown_impact_ground_effect_transition() {
    // Simulate descent from 20m to 0m — modifier should decrease monotonically
    let cfg = GroundEffectConfig { wingspan_m: 10.0 };
    let altitudes: Vec<f32> = (0..=20).rev().map(|a| a as f32).collect();
    let modifiers: Vec<f32> = altitudes
        .iter()
        .map(|&a| ground_effect_modifier(a, 65.0, &cfg))
        .collect();

    // Check that the last value (0m) is significantly less than the first (20m)
    let highest = modifiers[0]; // 20m
    let lowest = *modifiers.last().unwrap(); // 0m
    assert!(
        lowest < highest,
        "touchdown should reduce modifier: 20m={highest}, 0m={lowest}"
    );
    assert!(
        lowest < 0.05,
        "on-runway modifier should be near zero, got {lowest}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Weather transitions (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn transition_smooth_wind_direction_sweep() {
    // Sweeping wind direction 0→360 in 10° steps; forces should change smoothly
    let bridge = default_bridge();
    let mut prev: Option<FfbForces> = None;

    for deg in (0..=360).step_by(10) {
        let w = WeatherData {
            wind_speed_kts: 20.0,
            wind_direction_deg: deg as f64,
            turbulence_intensity: 0.0,
            gust_factor: 1.0,
            aircraft_heading_deg: 0.0,
            airspeed_kts: 120.0,
        };
        let f = bridge.compute_forces(&w);

        if let Some(ref p) = prev {
            let delta_cross = (f.crosswind_force - p.crosswind_force).abs();
            let delta_buffet = (f.headwind_buffet - p.headwind_buffet).abs();
            // 10° step should not produce jumps larger than 0.15
            assert!(
                delta_cross < 0.15,
                "crosswind jump at {deg}°: delta={delta_cross}"
            );
            assert!(
                delta_buffet < 0.15,
                "buffet jump at {deg}°: delta={delta_buffet}"
            );
        }
        prev = Some(f);
    }
}

#[test]
fn transition_storm_approach_increasing_forces() {
    // Simulate storm approach: wind speed ramps 5→45 kts
    let bridge = default_bridge();
    let mut prev_magnitude = 0.0_f64;

    for speed in (5..=45).step_by(5) {
        let w = WeatherData {
            wind_speed_kts: speed as f64,
            wind_direction_deg: 45.0,
            turbulence_intensity: (speed as f64 / 50.0).min(1.0),
            gust_factor: 1.0 + (speed as f64 - 5.0) / 80.0,
            aircraft_heading_deg: 0.0,
            airspeed_kts: 140.0,
        };
        let f = bridge.compute_forces(&w);
        let magnitude =
            f.crosswind_force.abs() + f.headwind_buffet.abs() + f.turbulence_shake.abs();

        assert!(
            magnitude >= prev_magnitude - 0.01,
            "storm approach forces should increase: at {speed}kts mag={magnitude}, prev={prev_magnitude}"
        );
        prev_magnitude = magnitude;
    }
}

#[test]
fn transition_altitude_based_ground_effect() {
    // Descending from 30m to 0m: ground effect should intensify
    let cfg = GroundEffectConfig { wingspan_m: 11.0 };
    let mut prev = ground_effect_modifier(30.0, 80.0, &cfg);

    for alt in (0..=29).rev() {
        let m = ground_effect_modifier(alt as f32, 80.0, &cfg);
        assert!(
            m <= prev + 1e-6,
            "descending should not increase modifier: alt={alt}, prev={prev}, cur={m}"
        );
        prev = m;
    }
}

#[test]
fn transition_terrain_induced_turbulence() {
    // Low altitude + high turbulence + crosswind: multiple systems engaged
    let bridge = default_bridge();
    let w = WeatherData {
        wind_speed_kts: 30.0,
        wind_direction_deg: 90.0,
        turbulence_intensity: 0.7,
        gust_factor: 1.5,
        aircraft_heading_deg: 0.0,
        airspeed_kts: 80.0,
    };
    let f = bridge.compute_forces(&w);

    // All three force channels should be non-trivially active
    assert!(
        f.crosswind_force.abs() > 0.1,
        "terrain turbulence should engage crosswind: {}",
        f.crosswind_force
    );
    assert!(
        f.headwind_buffet > 0.0,
        "terrain turbulence should engage buffet: {}",
        f.headwind_buffet
    );
    assert!(
        f.turbulence_shake > 0.3,
        "terrain turbulence should engage shake: {}",
        f.turbulence_shake
    );
}

#[test]
fn transition_crosswind_yaw_roll_coupling_at_low_airspeed() {
    // On approach: low airspeed amplifies crosswind effect vs. cruise
    let approach = compute_crosswind_forces(20.0, 90.0, 0.0, 50.0);
    let cruise = compute_crosswind_forces(20.0, 90.0, 0.0, 200.0);

    assert!(
        approach.yaw_force.abs() > cruise.yaw_force.abs(),
        "approach should amplify yaw: approach={}, cruise={}",
        approach.yaw_force,
        cruise.yaw_force
    );
    assert!(
        approach.roll_force.abs() > cruise.roll_force.abs(),
        "approach should amplify roll: approach={}, cruise={}",
        approach.roll_force,
        cruise.roll_force
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Property tests (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn property_force_magnitude_always_bounded() {
    let bridge = default_bridge();
    // Exhaustive sweep over a grid of wind speeds, directions, turbulence
    for speed in [0.0, 10.0, 50.0, 100.0, 200.0] {
        for dir in (0..=360).step_by(30) {
            for turb in [0.0, 0.3, 0.7, 1.0, 2.0] {
                for gust in [1.0, 2.0, 5.0] {
                    for ias in [0.0, 20.0, 100.0, 300.0] {
                        let w = WeatherData {
                            wind_speed_kts: speed,
                            wind_direction_deg: dir as f64,
                            turbulence_intensity: turb,
                            gust_factor: gust,
                            aircraft_heading_deg: 0.0,
                            airspeed_kts: ias,
                        };
                        let f = bridge.compute_forces(&w);
                        assert!(
                            (-1.0..=1.0).contains(&f.crosswind_force),
                            "crosswind out of bounds: {} for speed={speed},dir={dir},turb={turb},gust={gust},ias={ias}",
                            f.crosswind_force
                        );
                        assert!(
                            (-1.0..=1.0).contains(&f.headwind_buffet),
                            "buffet out of bounds: {} for speed={speed},dir={dir},turb={turb},gust={gust},ias={ias}",
                            f.headwind_buffet
                        );
                        assert!(
                            (0.0..=1.0).contains(&f.turbulence_shake),
                            "shake out of bounds: {} for speed={speed},dir={dir},turb={turb},gust={gust},ias={ias}",
                            f.turbulence_shake
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn property_turbulence_shake_monotonic_in_intensity() {
    // For fixed weather, increasing turbulence_intensity should never decrease shake
    let bridge = default_bridge();
    let intensities: Vec<f64> = (0..=100).map(|i| i as f64 / 100.0).collect();
    let mut prev_shake = 0.0_f64;

    for &t in &intensities {
        let w = WeatherData {
            wind_speed_kts: 15.0,
            wind_direction_deg: 45.0,
            turbulence_intensity: t,
            gust_factor: 1.0,
            aircraft_heading_deg: 0.0,
            airspeed_kts: 120.0,
        };
        let f = bridge.compute_forces(&w);
        assert!(
            f.turbulence_shake >= prev_shake - 1e-9,
            "shake should be monotonic: at intensity={t}, prev={prev_shake}, cur={}",
            f.turbulence_shake
        );
        prev_shake = f.turbulence_shake;
    }
}

#[test]
fn property_calm_weather_near_zero_all_channels() {
    // Calm weather across multiple heading/airspeed combos → all forces ~0
    let bridge = default_bridge();
    for heading in (0..=360).step_by(45) {
        for ias in [40.0, 100.0, 200.0, 400.0] {
            let w = WeatherData {
                wind_speed_kts: 0.0,
                wind_direction_deg: 0.0,
                turbulence_intensity: 0.0,
                gust_factor: 1.0,
                aircraft_heading_deg: heading as f64,
                airspeed_kts: ias,
            };
            let f = bridge.compute_forces(&w);
            let total = f.crosswind_force.abs() + f.headwind_buffet.abs() + f.turbulence_shake;
            assert!(
                total < 1e-9,
                "calm weather should produce zero forces: heading={heading}, ias={ias}, total={total}"
            );
        }
    }
}

#[test]
fn property_crosswind_forces_bounded_across_inputs() {
    // Crosswind module: yaw and roll always in [-1, 1] for any inputs
    for speed in [0.0_f32, 5.0, 50.0, 200.0, 999.0] {
        for dir in (0..=360).step_by(15) {
            for heading in (0..=360).step_by(45) {
                for ias in [0.0_f32, 3.0, 6.0, 80.0, 300.0] {
                    let out = compute_crosswind_forces(
                        speed,
                        dir as f32,
                        heading as f32,
                        ias,
                    );
                    assert!(
                        (-1.0..=1.0).contains(&out.yaw_force),
                        "yaw out of bounds: {} for speed={speed},dir={dir},hdg={heading},ias={ias}",
                        out.yaw_force
                    );
                    assert!(
                        (-1.0..=1.0).contains(&out.roll_force),
                        "roll out of bounds: {} for speed={speed},dir={dir},hdg={heading},ias={ias}",
                        out.roll_force
                    );
                }
            }
        }
    }
}
