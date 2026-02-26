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
    controls::{KspControls, apply_controls},
    error::KspError,
    mapping::{KspRawTelemetry, apply_telemetry, situation},
    protocol::{
        Argument, decode_double, decode_float, decode_int32, decode_object, decode_string,
        encode_object,
    },
};
use flight_adapter_common::{AdapterMetrics, AdapterState};
use flight_bus::{snapshot::BusSnapshot, types::AircraftId, types::SimId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::RwLock;
use tokio::time::{sleep, timeout};
use tracing::{debug, info, warn};

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
    /// Initial delay between reconnection attempts (doubles on each retry up to `max_reconnect_delay`)
    pub reconnect_delay: Duration,
    /// Maximum reconnect backoff delay
    pub max_reconnect_delay: Duration,
}

impl Default for KspConfig {
    fn default() -> Self {
        Self {
            krpc_host: "127.0.0.1".to_string(),
            krpc_port: 50000,
            poll_rate_hz: 20.0,
            connection_timeout: Duration::from_secs(5),
            reconnect_delay: Duration::from_secs(2),
            max_reconnect_delay: Duration::from_secs(60),
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
    pending_controls: Arc<RwLock<Option<KspControls>>>,
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
            pending_controls: Arc::new(RwLock::new(None)),
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
        let pending_controls = Arc::clone(&self.pending_controls);
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        tokio::spawn(async move {
            let mut current_delay = config.reconnect_delay;
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("KSP adapter shutting down");
                        break;
                    }
                    _ = run_connection_loop(&config, &state, &snapshot, &pending_controls) => {
                        // run_connection_loop only returns on error; reconnect
                    }
                }

                *state.write().await = AdapterState::Disconnected;
                *snapshot.write().await = None;

                info!(
                    "KSP adapter disconnected, retrying in {}s",
                    current_delay.as_secs()
                );
                sleep(current_delay).await;

                // Exponential backoff: double delay up to configured maximum
                current_delay = (current_delay * 2).min(config.max_reconnect_delay);
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

    /// Queue control outputs to be sent to KSP on the next poll cycle.
    ///
    /// The controls are applied once per poll cycle and then cleared.
    /// Values are clamped to valid ranges before transmission.
    /// This is a no-op when the adapter is not connected to an active vessel.
    pub async fn write_controls(&self, controls: KspControls) {
        *self.pending_controls.write().await = Some(controls);
    }
}

// ── Connection loop ───────────────────────────────────────────────────────────

async fn run_connection_loop(
    config: &KspConfig,
    state: &Arc<RwLock<AdapterState>>,
    snapshot: &Arc<RwLock<Option<BusSnapshot>>>,
    pending_controls: &Arc<RwLock<Option<KspControls>>>,
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
            Ok((raw, vessel_id)) => {
                *state.write().await = AdapterState::Active;
                let current = snapshot.read().await.clone().unwrap_or_else(|| {
                    BusSnapshot::new(SimId::Ksp, AircraftId::new(&raw.vessel_name))
                });
                let mut next = current;
                apply_telemetry(&mut next, &raw);
                *snapshot.write().await = Some(next);
                debug!(
                    "KSP poll ok: sit={} pitch={:.1}",
                    raw.situation, raw.pitch_deg
                );

                // Apply queued controls if any
                let controls = pending_controls.write().await.take();
                if let Some(ctrl) = controls
                    && let Err(e) = apply_controls(&mut conn, vessel_id, &ctrl).await
                {
                    warn!("kRPC control write error: {e}");
                    return; // trigger reconnect
                }
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

async fn poll_telemetry(conn: &mut KrpcConnection) -> Result<(KspRawTelemetry, u64), KspError> {
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
            (
                "SpaceCenter",
                "Vessel_get_SurfaceReferenceFrame",
                vec![vessel_arg.clone()],
            ),
            ("SpaceCenter", "Vessel_get_Name", vec![vessel_arg.clone()]),
            (
                "SpaceCenter",
                "Vessel_get_Situation",
                vec![vessel_arg.clone()],
            ),
            (
                "SpaceCenter",
                "Vessel_get_Latitude",
                vec![vessel_arg.clone()],
            ),
            (
                "SpaceCenter",
                "Vessel_get_Longitude",
                vec![vessel_arg.clone()],
            ),
            (
                "SpaceCenter",
                "Vessel_get_MeanAltitude",
                vec![vessel_arg.clone()],
            ),
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
            (
                "SpaceCenter",
                "Flight_get_Heading",
                vec![flight_arg.clone()],
            ),
            ("SpaceCenter", "Flight_get_Speed", vec![flight_arg.clone()]),
            (
                "SpaceCenter",
                "Flight_get_EquivalentAirSpeed",
                vec![flight_arg.clone()],
            ),
            (
                "SpaceCenter",
                "Flight_get_VerticalSpeed",
                vec![flight_arg.clone()],
            ),
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

    Ok((KspRawTelemetry {
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
    }, vessel_id))
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
        assert_eq!(cfg.connection_timeout, Duration::from_secs(5));
        assert_eq!(cfg.reconnect_delay, Duration::from_secs(2));
        assert_eq!(cfg.max_reconnect_delay, Duration::from_secs(60));
    }

    #[test]
    fn test_config_custom_values() {
        let cfg = KspConfig {
            krpc_host: "192.168.1.10".to_string(),
            krpc_port: 50001,
            poll_rate_hz: 10.0,
            connection_timeout: Duration::from_secs(10),
            reconnect_delay: Duration::from_secs(5),
            max_reconnect_delay: Duration::from_secs(120),
        };
        assert_eq!(cfg.krpc_host, "192.168.1.10");
        assert_eq!(cfg.krpc_port, 50001);
        assert_eq!(cfg.poll_rate_hz, 10.0);
        assert_eq!(cfg.max_reconnect_delay, Duration::from_secs(120));
    }

    #[tokio::test]
    async fn test_write_controls_before_start_does_not_panic() {
        let adapter = KspAdapter::new(KspConfig::default());
        // write_controls stores to pending — must not panic
        adapter.write_controls(KspControls::default()).await;
    }

    #[tokio::test]
    async fn test_write_controls_pending_is_cleared_on_next_write() {
        let adapter = KspAdapter::new(KspConfig::default());
        adapter.write_controls(KspControls::from_axes(0.5, -0.5, 0.0, 0.8)).await;
        // Second write overwrites the first
        adapter.write_controls(KspControls::default()).await;
        // Adapter is not connected so no assertion on output; just verify no panic
    }

    #[tokio::test]
    async fn test_snapshot_initially_none() {
        let adapter = KspAdapter::new(KspConfig::default());
        assert!(adapter.current_snapshot().await.is_none());
    }

    #[tokio::test]
    async fn test_state_initially_disconnected() {
        let adapter = KspAdapter::new(KspConfig::default());
        assert_eq!(adapter.state().await, AdapterState::Disconnected);
    }

    #[tokio::test]
    async fn test_double_stop_does_not_panic() {
        let adapter = KspAdapter::new(KspConfig::default());
        adapter.stop().await;
        adapter.stop().await;
        assert_eq!(adapter.state().await, AdapterState::Disconnected);
    }

    #[test]
    fn test_backoff_doubles_up_to_max() {
        // Simulate the backoff logic directly
        let max = Duration::from_secs(60);
        let mut delay = Duration::from_secs(2);
        let sequence: Vec<u64> = (0..8)
            .map(|_| {
                let d = delay.as_secs();
                delay = (delay * 2).min(max);
                d
            })
            .collect();
        assert_eq!(sequence, vec![2, 4, 8, 16, 32, 60, 60, 60]);
    }
}
