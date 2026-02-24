//! KSP kRPC adapter — connects to a running Kerbal Space Program instance via
//! the kRPC mod and publishes telemetry onto the Flight Hub bus.
//!
//! ## Usage
//!
//! 1. Install the kRPC mod in KSP (see <https://krpc.github.io/krpc/>).
//! 2. Start KSP and load a vessel.
//! 3. Create `KspAdapter::new(KspConfig::default())` and call `start()`.
//!
//! The adapter automatically reconnects when KSP is restarted.

use crate::{
    connection::KrpcConnection,
    error::KspError,
    mapping::{apply_telemetry, situation, KspRawTelemetry},
    protocol::{decode_double, decode_float, decode_int32, decode_object, decode_string, encode_object, Argument},
};
use flight_adapter_common::{AdapterMetrics, AdapterState};
use flight_bus::{snapshot::BusSnapshot, types::AircraftId, types::SimId};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};
use std::sync::Arc;

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for the KSP kRPC adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KspConfig {
    /// kRPC server hostname or IP address (default: "127.0.0.1")
    pub krpc_host: String,
    /// kRPC RPC port (default: 50000)
    pub krpc_port: u16,
    /// Telemetry poll rate in Hz (default: 20 Hz)
    pub poll_rate_hz: f32,
    /// TCP connection timeout
    pub connection_timeout: Duration,
    /// Delay between reconnection attempts
    pub reconnect_delay: Duration,
}

impl Default for KspConfig {
    fn default() -> Self {
        Self {
            krpc_host: "127.0.0.1".to_string(),
            krpc_port: 50000,
            poll_rate_hz: 20.0,
            connection_timeout: Duration::from_secs(5),
            reconnect_delay: Duration::from_secs(2),
        }
    }
}

// ── Adapter ───────────────────────────────────────────────────────────────────

/// KSP adapter — manages the kRPC connection lifecycle and polls vessel
/// telemetry into a shared [`BusSnapshot`].
pub struct KspAdapter {
    config: KspConfig,
    state: Arc<RwLock<AdapterState>>,
    snapshot: Arc<RwLock<Option<BusSnapshot>>>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    #[allow(dead_code)]
    metrics: Arc<RwLock<AdapterMetrics>>,
}

impl KspAdapter {
    /// Create a new adapter with the given configuration.
    pub fn new(config: KspConfig) -> Self {
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
        Self {
            config,
            state: Arc::new(RwLock::new(AdapterState::Disconnected)),
            snapshot: Arc::new(RwLock::new(None)),
            shutdown_tx,
            metrics: Arc::new(RwLock::new(AdapterMetrics::new())),
        }
    }

    /// Returns `SimId::Ksp`.
    pub fn sim_id(&self) -> SimId {
        SimId::Ksp
    }

    /// Start the adapter background task.
    pub async fn start(&self) {
        let config = self.config.clone();
        let state = Arc::clone(&self.state);
        let snapshot = Arc::clone(&self.snapshot);
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("KSP adapter shutting down");
                        break;
                    }
                    _ = run_connection_loop(&config, &state, &snapshot) => {
                        // run_connection_loop only returns on error; reconnect
                    }
                }

                *state.write().await = AdapterState::Disconnected;
                *snapshot.write().await = None;

                info!(
                    "KSP adapter disconnected, retrying in {}s",
                    config.reconnect_delay.as_secs()
                );
                sleep(config.reconnect_delay).await;
            }
        });
    }

    /// Stop the adapter.
    pub async fn stop(&self) {
        let _ = self.shutdown_tx.send(());
        *self.state.write().await = AdapterState::Disconnected;
        *self.snapshot.write().await = None;
    }

    /// Returns the most recent telemetry snapshot, or `None` if not yet
    /// connected / no active vessel.
    pub async fn current_snapshot(&self) -> Option<BusSnapshot> {
        self.snapshot.read().await.clone()
    }

    /// Returns the current adapter lifecycle state.
    pub async fn state(&self) -> AdapterState {
        *self.state.read().await
    }
}

// ── Connection loop ───────────────────────────────────────────────────────────

async fn run_connection_loop(
    config: &KspConfig,
    state: &Arc<RwLock<AdapterState>>,
    snapshot: &Arc<RwLock<Option<BusSnapshot>>>,
) {
    *state.write().await = AdapterState::Connecting;

    let conn_result = timeout(
        config.connection_timeout,
        KrpcConnection::connect(&config.krpc_host, config.krpc_port, "OpenFlight"),
    )
    .await;

    let mut conn = match conn_result {
        Ok(Ok(c)) => {
            info!(
                "Connected to kRPC at {}:{}",
                config.krpc_host, config.krpc_port
            );
            *state.write().await = AdapterState::Connected;
            c
        }
        Ok(Err(e)) => {
            warn!("kRPC connection failed: {e}");
            return;
        }
        Err(_) => {
            warn!("kRPC connection timed out");
            return;
        }
    };

    let poll_interval = Duration::from_secs_f32(1.0 / config.poll_rate_hz.max(0.1));
    let mut last_poll = Instant::now();

    loop {
        let elapsed = last_poll.elapsed();
        if elapsed < poll_interval {
            sleep(poll_interval - elapsed).await;
        }
        last_poll = Instant::now();

        match poll_telemetry(&mut conn).await {
            Ok(raw) => {
                *state.write().await = AdapterState::Active;
                let current = snapshot.read().await.clone().unwrap_or_else(|| {
                    BusSnapshot::new(SimId::Ksp, AircraftId::new(&raw.vessel_name))
                });
                let mut next = current;
                apply_telemetry(&mut next, &raw);
                *snapshot.write().await = Some(next);
                debug!("KSP poll ok: sit={} pitch={:.1}", raw.situation, raw.pitch_deg);
            }
            Err(KspError::NoActiveVessel) => {
                *state.write().await = AdapterState::Connected;
                *snapshot.write().await = None;
                debug!("No active KSP vessel");
            }
            Err(e) => {
                warn!("kRPC poll error: {e}");
                return; // trigger reconnect
            }
        }
    }
}

// ── Telemetry polling ─────────────────────────────────────────────────────────

async fn poll_telemetry(conn: &mut KrpcConnection) -> Result<KspRawTelemetry, KspError> {
    // 1. Get active vessel handle
    let vessel_bytes = conn.call("SpaceCenter", "get_ActiveVessel", vec![]).await?;
    let vessel_id = decode_object(&vessel_bytes).unwrap_or(0);
    if vessel_id == 0 {
        return Err(KspError::NoActiveVessel);
    }
    let vessel_arg = Argument {
        position: 0,
        value: encode_object(vessel_id),
    };

    // 2. Batch: get surface reference frame + vessel name + situation + lat/lon + altitude
    let step2 = conn
        .call_batch(vec![
            ("SpaceCenter", "Vessel_get_SurfaceReferenceFrame", vec![vessel_arg.clone()]),
            ("SpaceCenter", "Vessel_get_Name", vec![vessel_arg.clone()]),
            ("SpaceCenter", "Vessel_get_Situation", vec![vessel_arg.clone()]),
            ("SpaceCenter", "Vessel_get_Latitude", vec![vessel_arg.clone()]),
            ("SpaceCenter", "Vessel_get_Longitude", vec![vessel_arg.clone()]),
            ("SpaceCenter", "Vessel_get_MeanAltitude", vec![vessel_arg.clone()]),
        ])
        .await?;

    let ref_frame_id = decode_object(&step2[0]).unwrap_or(0);
    let vessel_name = decode_string(&step2[1]).unwrap_or_else(|_| "Unknown".to_string());
    let situation = decode_int32(&step2[2]).unwrap_or(situation::LANDED);
    let latitude_deg = decode_double(&step2[3]).unwrap_or(0.0);
    let longitude_deg = decode_double(&step2[4]).unwrap_or(0.0);
    let altitude_m = decode_double(&step2[5]).unwrap_or(0.0);

    // 3. Get flight object (vessel + surface reference frame)
    let ref_frame_arg = Argument {
        position: 1,
        value: encode_object(ref_frame_id),
    };
    let flight_bytes = conn
        .call(
            "SpaceCenter",
            "Vessel_get_Flight",
            vec![vessel_arg, ref_frame_arg],
        )
        .await?;
    let flight_id = decode_object(&flight_bytes).unwrap_or(0);
    let flight_arg = Argument {
        position: 0,
        value: encode_object(flight_id),
    };

    // 4. Batch: attitude + kinematics
    let step4 = conn
        .call_batch(vec![
            ("SpaceCenter", "Flight_get_Pitch", vec![flight_arg.clone()]),
            ("SpaceCenter", "Flight_get_Roll", vec![flight_arg.clone()]),
            ("SpaceCenter", "Flight_get_Heading", vec![flight_arg.clone()]),
            ("SpaceCenter", "Flight_get_Speed", vec![flight_arg.clone()]),
            ("SpaceCenter", "Flight_get_EquivalentAirSpeed", vec![flight_arg.clone()]),
            ("SpaceCenter", "Flight_get_VerticalSpeed", vec![flight_arg.clone()]),
            ("SpaceCenter", "Flight_get_GForce", vec![flight_arg.clone()]),
        ])
        .await?;

    let pitch_deg = decode_float(&step4[0]).unwrap_or(0.0);
    let roll_deg = decode_float(&step4[1]).unwrap_or(0.0);
    let heading_deg = decode_float(&step4[2]).unwrap_or(0.0);
    let speed_mps = decode_double(&step4[3]).unwrap_or(0.0);
    let ias_mps = decode_double(&step4[4]).unwrap_or(0.0);
    let vertical_speed_mps = decode_double(&step4[5]).unwrap_or(0.0);
    let g_force = decode_double(&step4[6]).unwrap_or(1.0);

    Ok(KspRawTelemetry {
        vessel_name,
        situation,
        pitch_deg,
        roll_deg,
        heading_deg,
        speed_mps,
        ias_mps,
        vertical_speed_mps,
        g_force,
        altitude_m,
        latitude_deg,
        longitude_deg,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adapter_creation() {
        let adapter = KspAdapter::new(KspConfig::default());
        assert_eq!(adapter.sim_id(), SimId::Ksp);
        assert_eq!(adapter.state().await, AdapterState::Disconnected);
        assert!(adapter.current_snapshot().await.is_none());
    }

    #[tokio::test]
    async fn test_adapter_stop_without_start() {
        let adapter = KspAdapter::new(KspConfig::default());
        // stop() before start() should not panic
        adapter.stop().await;
        assert_eq!(adapter.state().await, AdapterState::Disconnected);
    }

    #[test]
    fn test_config_defaults() {
        let cfg = KspConfig::default();
        assert_eq!(cfg.krpc_host, "127.0.0.1");
        assert_eq!(cfg.krpc_port, 50000);
        assert_eq!(cfg.poll_rate_hz, 20.0);
    }
}
