//! KSP control output — map axis engine values to kRPC control surface inputs.
//!
//! The kRPC `Control` service accepts pitch/roll/yaw in the range −1.0…+1.0
//! and throttle in the range 0.0…+1.0.  Values outside these ranges are clamped
//! before being sent.

use crate::{
    connection::KrpcConnection,
    error::KspError,
    protocol::{Argument, decode_object, encode_bool, encode_float, encode_object},
};

// ── Control state ─────────────────────────────────────────────────────────────

/// Axis outputs to be written to KSP via kRPC in one update cycle.
///
/// All axes are clamped to their valid ranges before transmission.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct KspControls {
    /// Pitch deflection in −1.0 (full forward) … +1.0 (full back).
    pub pitch: f32,
    /// Roll deflection in −1.0 (full left) … +1.0 (full right).
    pub roll: f32,
    /// Yaw deflection in −1.0 (full left) … +1.0 (full right).
    pub yaw: f32,
    /// Throttle in 0.0 (idle) … 1.0 (full power).
    pub throttle: f32,
    /// Landing gear state — `Some(true)` = deployed, `Some(false)` = retracted,
    /// `None` = leave unchanged.
    pub gear: Option<bool>,
}

impl KspControls {
    /// Construct from individual axis values (no gear change).
    pub fn from_axes(pitch: f32, roll: f32, yaw: f32, throttle: f32) -> Self {
        Self {
            pitch,
            roll,
            yaw,
            throttle,
            gear: None,
        }
    }

    /// Return a copy with all values clamped to their valid ranges.
    /// Non-finite values (NaN, Inf) are coerced to the safe default (0.0)
    /// before clamping.
    pub fn clamped(&self) -> Self {
        let sanitize = |v: f32, default: f32| if v.is_finite() { v } else { default };
        Self {
            pitch: sanitize(self.pitch, 0.0).clamp(-1.0, 1.0),
            roll: sanitize(self.roll, 0.0).clamp(-1.0, 1.0),
            yaw: sanitize(self.yaw, 0.0).clamp(-1.0, 1.0),
            throttle: sanitize(self.throttle, 0.0).clamp(0.0, 1.0),
            gear: self.gear,
        }
    }

    /// Return `true` if all axes are within their valid ranges (no clamping needed).
    pub fn is_valid(&self) -> bool {
        (-1.0..=1.0).contains(&self.pitch)
            && (-1.0..=1.0).contains(&self.roll)
            && (-1.0..=1.0).contains(&self.yaw)
            && (0.0..=1.0).contains(&self.throttle)
    }
}

// ── kRPC write ────────────────────────────────────────────────────────────────

/// Write `controls` to the active vessel via an open kRPC connection.
///
/// Fetches the `Control` object handle from the vessel, then batches
/// pitch/roll/yaw/throttle writes into a single round-trip.  If
/// `controls.gear` is `Some`, an additional call sets the landing gear state.
pub async fn apply_controls(
    conn: &mut KrpcConnection,
    vessel_id: u64,
    controls: &KspControls,
) -> Result<(), KspError> {
    let c = controls.clamped();

    let vessel_arg = Argument {
        position: 0,
        value: encode_object(vessel_id),
    };

    // Fetch the Control object handle
    let control_bytes = conn
        .call("SpaceCenter", "Vessel_get_Control", vec![vessel_arg])
        .await?;
    let control_id = decode_object(&control_bytes).unwrap_or(0);
    if control_id == 0 {
        return Err(KspError::Protocol(
            "Failed to get Control handle".to_string(),
        ));
    }
    let ctrl = |pos: u32| Argument {
        position: pos,
        value: encode_object(control_id),
    };

    // Batch: pitch, roll, yaw, throttle
    conn.call_batch(vec![
        (
            "SpaceCenter",
            "Control_set_Pitch",
            vec![
                ctrl(0),
                Argument {
                    position: 1,
                    value: encode_float(c.pitch),
                },
            ],
        ),
        (
            "SpaceCenter",
            "Control_set_Roll",
            vec![
                ctrl(0),
                Argument {
                    position: 1,
                    value: encode_float(c.roll),
                },
            ],
        ),
        (
            "SpaceCenter",
            "Control_set_Yaw",
            vec![
                ctrl(0),
                Argument {
                    position: 1,
                    value: encode_float(c.yaw),
                },
            ],
        ),
        (
            "SpaceCenter",
            "Control_set_Throttle",
            vec![
                ctrl(0),
                Argument {
                    position: 1,
                    value: encode_float(c.throttle),
                },
            ],
        ),
    ])
    .await?;

    // Gear (separate call — less frequent)
    if let Some(gear_down) = c.gear {
        conn.call(
            "SpaceCenter",
            "Control_set_Gear",
            vec![
                ctrl(0),
                Argument {
                    position: 1,
                    value: encode_bool(gear_down),
                },
            ],
        )
        .await?;
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_controls_are_zero() {
        let c = KspControls::default();
        assert_eq!(c.pitch, 0.0);
        assert_eq!(c.roll, 0.0);
        assert_eq!(c.yaw, 0.0);
        assert_eq!(c.throttle, 0.0);
        assert!(c.gear.is_none());
    }

    #[test]
    fn from_axes_sets_fields_correctly() {
        let c = KspControls::from_axes(0.5, -0.3, 0.1, 0.8);
        assert_eq!(c.pitch, 0.5);
        assert_eq!(c.roll, -0.3);
        assert_eq!(c.yaw, 0.1);
        assert_eq!(c.throttle, 0.8);
        assert!(c.gear.is_none());
    }

    #[test]
    fn is_valid_accepts_in_range_values() {
        let c = KspControls::from_axes(1.0, -1.0, 0.0, 0.5);
        assert!(c.is_valid());
    }

    #[test]
    fn is_valid_rejects_pitch_over_one() {
        let c = KspControls {
            pitch: 1.1,
            ..Default::default()
        };
        assert!(!c.is_valid());
    }

    #[test]
    fn is_valid_rejects_negative_throttle() {
        let c = KspControls {
            throttle: -0.1,
            ..Default::default()
        };
        assert!(!c.is_valid());
    }

    #[test]
    fn is_valid_rejects_throttle_over_one() {
        let c = KspControls {
            throttle: 1.5,
            ..Default::default()
        };
        assert!(!c.is_valid());
    }

    #[test]
    fn clamped_fixes_out_of_range_pitch() {
        let c = KspControls {
            pitch: 2.5,
            ..Default::default()
        };
        let clamped = c.clamped();
        assert_eq!(clamped.pitch, 1.0);
        assert!(clamped.is_valid());
    }

    #[test]
    fn clamped_fixes_negative_throttle() {
        let c = KspControls {
            throttle: -0.5,
            ..Default::default()
        };
        let clamped = c.clamped();
        assert_eq!(clamped.throttle, 0.0);
        assert!(clamped.is_valid());
    }

    #[test]
    fn clamped_preserves_gear_option() {
        let c = KspControls {
            gear: Some(true),
            throttle: 2.0,
            ..Default::default()
        };
        let clamped = c.clamped();
        assert_eq!(clamped.gear, Some(true));
        assert_eq!(clamped.throttle, 1.0);
    }

    #[test]
    fn clamped_identity_when_already_valid() {
        let c = KspControls::from_axes(0.5, -0.5, 0.25, 0.75);
        let clamped = c.clamped();
        assert_eq!(clamped.pitch, c.pitch);
        assert_eq!(clamped.roll, c.roll);
        assert_eq!(clamped.yaw, c.yaw);
        assert_eq!(clamped.throttle, c.throttle);
    }
}
