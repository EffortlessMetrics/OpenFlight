//! Maps raw kRPC telemetry values to `BusSnapshot` fields.
//!
//! ## kRPC value units
//!
//! | kRPC procedure | Unit | BusSnapshot field | Conversion |
//! |---|---|---|---|
//! | Flight_get_Pitch | degrees | kinematics.pitch | ValidatedAngle::new_degrees |
//! | Flight_get_Roll | degrees | kinematics.bank | ValidatedAngle::new_degrees |
//! | Flight_get_Heading | degrees 0-360 | kinematics.heading | normalize to −180…+180 |
//! | Flight_get_Speed | m/s | kinematics.tas | ×1.94384 → knots |
//! | Flight_get_EquivalentAirSpeed | m/s | kinematics.ias | ×1.94384 → knots |
//! | Flight_get_VerticalSpeed | m/s | kinematics.vertical_speed | ×196.85 → fpm |
//! | Flight_get_GForce | g | kinematics.g_force | direct |
//! | Vessel_get_MeanAltitude | m | environment.altitude | ×3.28084 → feet |
//! | Vessel_get_Latitude | degrees | navigation.latitude | direct |
//! | Vessel_get_Longitude | degrees | navigation.longitude | direct |

use flight_bus::{
    snapshot::{BusSnapshot, Environment, Navigation, ValidityFlags},
    types::{AircraftId, GForce, SimId, ValidatedAngle, ValidatedSpeed},
};

/// Raw telemetry values fetched from kRPC in a single poll cycle.
#[derive(Debug, Clone, Default)]
pub struct KspRawTelemetry {
    pub vessel_name: String,
    /// KSP VesselSituation enum value
    pub situation: i32,
    /// pitch angle (degrees, −90…+90, positive = nose up)
    pub pitch_deg: f32,
    /// roll/bank angle (degrees, −180…+180)
    pub roll_deg: f32,
    /// heading (degrees, 0…360)
    pub heading_deg: f32,
    /// true/orbital speed (m/s)
    pub speed_mps: f64,
    /// equivalent airspeed (m/s) — only meaningful in atmosphere
    pub ias_mps: f64,
    /// vertical speed (m/s, positive = climbing)
    pub vertical_speed_mps: f64,
    /// g-force (g)
    pub g_force: f64,
    /// mean altitude above sea level (m)
    pub altitude_m: f64,
    /// geodetic latitude (degrees)
    pub latitude_deg: f64,
    /// geodetic longitude (degrees)
    pub longitude_deg: f64,
}

/// KSP VesselSituation enum values as reported by kRPC.
pub mod situation {
    pub const LANDED: i32 = 0;
    pub const SPLASHED: i32 = 1;
    pub const PRELAUNCH: i32 = 2;
    pub const FLYING: i32 = 3;
    pub const SUB_ORBITAL: i32 = 4;
    pub const ORBITING: i32 = 5;
    pub const ESCAPING: i32 = 6;
    pub const DOCKED: i32 = 7;
}

const MPS_TO_KNOTS: f64 = 1.943_844;
const MPS_TO_FPM: f64 = 196.850_394;
const M_TO_FEET: f64 = 3.280_840;

/// Populate `snapshot` from `raw`. All out-of-range values are clamped/skipped
/// rather than panicking so the adapter degrades gracefully.
pub fn apply_telemetry(snapshot: &mut BusSnapshot, raw: &KspRawTelemetry) {
    // ── Kinematics ───────────────────────────────────────────────────────────
    if let Ok(pitch) = ValidatedAngle::new_degrees(raw.pitch_deg) {
        snapshot.kinematics.pitch = pitch;
    }
    if let Ok(bank) = ValidatedAngle::new_degrees(raw.roll_deg) {
        snapshot.kinematics.bank = bank;
    }
    // Normalize 0-360 heading to -180…+180 expected by ValidatedAngle
    let heading_normalized = if raw.heading_deg > 180.0 {
        raw.heading_deg - 360.0
    } else {
        raw.heading_deg
    };
    if let Ok(hdg) = ValidatedAngle::new_degrees(heading_normalized) {
        snapshot.kinematics.heading = hdg;
    }

    let speed_kt = (raw.speed_mps * MPS_TO_KNOTS) as f32;
    if let Ok(tas) = ValidatedSpeed::new_knots(speed_kt.clamp(0.0, 1000.0)) {
        snapshot.kinematics.tas = tas;
    }

    let ias_kt = (raw.ias_mps * MPS_TO_KNOTS) as f32;
    if let Ok(ias) = ValidatedSpeed::new_knots(ias_kt.clamp(0.0, 1000.0)) {
        snapshot.kinematics.ias = ias;
    }

    snapshot.kinematics.vertical_speed = (raw.vertical_speed_mps * MPS_TO_FPM) as f32;

    let g_clamped = raw.g_force.clamp(-20.0, 20.0) as f32;
    if let Ok(g) = GForce::new(g_clamped) {
        snapshot.kinematics.g_force = g;
    }

    // ── Environment ──────────────────────────────────────────────────────────
    snapshot.environment = Environment {
        altitude: (raw.altitude_m * M_TO_FEET) as f32,
        ..Environment::default()
    };

    // ── Navigation ───────────────────────────────────────────────────────────
    snapshot.navigation = Navigation {
        latitude: raw.latitude_deg,
        longitude: raw.longitude_deg,
        ..Navigation::default()
    };

    // ── Validity ─────────────────────────────────────────────────────────────
    let in_atmosphere = raw.situation == situation::FLYING;
    let in_flight = raw.situation >= situation::FLYING;
    snapshot.validity = ValidityFlags {
        safe_for_ffb: in_atmosphere,
        attitude_valid: in_flight,
        velocities_valid: in_flight,
        kinematics_valid: in_atmosphere,
        aero_valid: in_atmosphere,
        position_valid: true,
        angular_rates_valid: false, // not yet fetched
    };

    // Use the vessel name as the aircraft ICAO identifier
    snapshot.aircraft = AircraftId::new(raw.vessel_name.as_str());
    snapshot.sim = SimId::Ksp;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_bus::{snapshot::BusSnapshot, types::SimId};

    fn default_snapshot() -> BusSnapshot {
        BusSnapshot::new(SimId::Ksp, AircraftId::new("test"))
    }

    #[test]
    fn test_pitch_mapping() {
        let mut snap = default_snapshot();
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                pitch_deg: 15.0,
                situation: situation::FLYING,
                ..Default::default()
            },
        );
        assert!((snap.kinematics.pitch.to_degrees() - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_heading_normalization() {
        let mut snap = default_snapshot();
        // 270 degrees should become -90 after normalization
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                heading_deg: 270.0,
                situation: situation::FLYING,
                ..Default::default()
            },
        );
        assert!((snap.kinematics.heading.to_degrees() - (-90.0)).abs() < 0.01);
    }

    #[test]
    fn test_altitude_conversion() {
        let mut snap = default_snapshot();
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                altitude_m: 1000.0, // 1000 m
                situation: situation::FLYING,
                ..Default::default()
            },
        );
        // 1000 m × 3.28084 ≈ 3280.84 ft
        assert!((snap.environment.altitude - 3280.84).abs() < 1.0);
    }

    #[test]
    fn test_safe_for_ffb_only_when_flying() {
        let mut snap = default_snapshot();
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                situation: situation::LANDED,
                ..Default::default()
            },
        );
        assert!(!snap.validity.safe_for_ffb);

        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                situation: situation::FLYING,
                ..Default::default()
            },
        );
        assert!(snap.validity.safe_for_ffb);
    }

    #[test]
    fn test_speed_conversion() {
        let mut snap = default_snapshot();
        // 100 m/s ≈ 194.4 knots
        apply_telemetry(
            &mut snap,
            &KspRawTelemetry {
                speed_mps: 100.0,
                ias_mps: 80.0,
                situation: situation::FLYING,
                ..Default::default()
            },
        );
        assert!((snap.kinematics.tas.to_knots() - 194.38).abs() < 0.5);
        assert!((snap.kinematics.ias.to_knots() - 155.51).abs() < 0.5);
    }
}
