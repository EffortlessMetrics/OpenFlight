// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Ace Combat 7 telemetry UDP adapter.
//!
//! This adapter consumes externally-provided AC7 telemetry packets and publishes
//! normalized snapshots to the Flight Hub bus.

use flight_ac7_protocol::{Ac7ProtocolError, Ac7TelemetryPacket};
use flight_adapter_common::{AdapterConfig, AdapterError, AdapterMetrics, AdapterState};
use flight_bus::{
    BusPublisher, BusSnapshot, PublisherError,
    types::{AircraftId, GForce, SimId, ValidatedAngle, ValidatedSpeed},
};
use flight_core::units::{angles, conversions};
use flight_metrics::{
    MetricsRegistry,
    common::{
        ADAPTER_ERRORS_TOTAL, ADAPTER_TIME_SINCE_LAST_PACKET_MS, ADAPTER_UPDATE_LATENCY_MS,
        ADAPTER_UPDATES_TOTAL,
    },
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use tracing::{debug, info};

/// AC7 telemetry adapter config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ac7TelemetryConfig {
    /// UDP bind address for telemetry bridge packets.
    pub listen_addr: SocketAddr,
    /// Bus publish max rate.
    pub bus_max_rate_hz: f32,
    /// Expected packet update rate.
    pub update_rate_hz: f32,
    /// Timeout for packet reception.
    pub connection_timeout: Duration,
    /// Maximum accepted packet size.
    pub max_packet_size: usize,
}

impl Default for Ac7TelemetryConfig {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:7779"
                .parse()
                .expect("hardcoded address must be valid"),
            bus_max_rate_hz: 60.0,
            update_rate_hz: 60.0,
            connection_timeout: Duration::from_secs(2),
            max_packet_size: 4096,
        }
    }
}

impl AdapterConfig for Ac7TelemetryConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.update_rate_hz
    }

    fn connection_timeout(&self) -> Duration {
        self.connection_timeout
    }

    fn max_reconnect_attempts(&self) -> u32 {
        0
    }

    fn enable_auto_reconnect(&self) -> bool {
        false
    }
}

/// Adapter errors.
#[derive(Debug, Error)]
pub enum Ac7TelemetryError {
    #[error("adapter is not started")]
    NotStarted,
    #[error("socket error: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol error: {0}")]
    Protocol(#[from] Ac7ProtocolError),
    #[error("publisher error: {0}")]
    Publisher(#[from] PublisherError),
    #[error(transparent)]
    Adapter(#[from] AdapterError),
    #[error("invalid telemetry field: {field}")]
    InvalidField { field: &'static str },
}

/// AC7 telemetry adapter.
pub struct Ac7TelemetryAdapter {
    config: Ac7TelemetryConfig,
    socket: Option<UdpSocket>,
    bus_publisher: BusPublisher,
    state: AdapterState,
    metrics: AdapterMetrics,
    metrics_registry: MetricsRegistry,
    started_at: Instant,
    last_packet: Option<Instant>,
    source_addr: Option<SocketAddr>,
}

impl Ac7TelemetryAdapter {
    /// Create a new adapter.
    pub fn new(config: Ac7TelemetryConfig) -> Self {
        Self {
            bus_publisher: BusPublisher::new(config.bus_max_rate_hz),
            config,
            socket: None,
            state: AdapterState::Disconnected,
            metrics: AdapterMetrics::new(),
            metrics_registry: MetricsRegistry::new(),
            started_at: Instant::now(),
            last_packet: None,
            source_addr: None,
        }
    }

    /// Start adapter and bind UDP socket.
    pub async fn start(&mut self) -> Result<(), Ac7TelemetryError> {
        self.state = AdapterState::Connecting;
        let socket = UdpSocket::bind(self.config.listen_addr).await?;
        let bound = socket.local_addr()?;
        self.socket = Some(socket);
        self.state = AdapterState::Connected;
        info!("AC7 telemetry adapter listening on {}", bound);
        Ok(())
    }

    /// Stop adapter.
    pub fn stop(&mut self) {
        self.socket = None;
        self.state = AdapterState::Disconnected;
    }

    /// Poll and process at most one telemetry packet.
    pub async fn poll_once(&mut self) -> Result<Option<BusSnapshot>, Ac7TelemetryError> {
        let Some(socket) = self.socket.as_ref() else {
            return Err(Ac7TelemetryError::NotStarted);
        };

        let mut buffer = vec![0u8; self.config.max_packet_size];
        let (size, addr) = timeout(
            self.config.connection_timeout,
            socket.recv_from(&mut buffer),
        )
        .await
        .map_err(|_| AdapterError::Timeout("AC7 packet timeout".into()))??;

        let update_start = Instant::now();
        let packet = Ac7TelemetryPacket::from_json_slice(&buffer[..size]).map_err(|e| {
            self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            e
        })?;
        let snapshot = self.convert_packet_to_snapshot(&packet)?;
        self.bus_publisher.publish(snapshot.clone()).map_err(|e| {
            self.metrics_registry.inc_counter(ADAPTER_ERRORS_TOTAL, 1);
            e
        })?;

        self.state = AdapterState::Active;
        self.last_packet = Some(Instant::now());
        self.source_addr = Some(addr);
        self.metrics.record_update();
        self.metrics
            .record_aircraft_change(packet.aircraft_label().to_string());
        self.metrics_registry.inc_counter(ADAPTER_UPDATES_TOTAL, 1);
        self.metrics_registry.observe(
            ADAPTER_UPDATE_LATENCY_MS,
            update_start.elapsed().as_secs_f64() * 1000.0,
        );

        if let Some(last) = self.last_packet {
            self.metrics_registry.set_gauge(
                ADAPTER_TIME_SINCE_LAST_PACKET_MS,
                Instant::now().duration_since(last).as_secs_f64() * 1000.0,
            );
        }

        debug!("AC7 telemetry packet processed from {}", addr);
        Ok(Some(snapshot))
    }

    /// Convert a wire packet into a bus snapshot.
    pub fn convert_packet_to_snapshot(
        &self,
        packet: &Ac7TelemetryPacket,
    ) -> Result<BusSnapshot, Ac7TelemetryError> {
        let mut snapshot = BusSnapshot::new(
            SimId::AceCombat7,
            AircraftId::new(packet.aircraft_label().to_string()),
        );
        snapshot.timestamp = Instant::now().duration_since(self.started_at).as_nanos() as u64;

        if let Some(speed_mps) = packet.state.speed_mps {
            let speed = ValidatedSpeed::new_mps(speed_mps).map_err(|_| {
                Ac7TelemetryError::InvalidField {
                    field: "state.speed_mps",
                }
            })?;
            snapshot.kinematics.ias = speed;
            snapshot.kinematics.tas = speed;
            if packet.state.ground_speed_mps.is_none() {
                snapshot.kinematics.ground_speed = speed;
            }
        }

        if let Some(ground_speed_mps) = packet.state.ground_speed_mps {
            snapshot.kinematics.ground_speed =
                ValidatedSpeed::new_mps(ground_speed_mps).map_err(|_| {
                    Ac7TelemetryError::InvalidField {
                        field: "state.ground_speed_mps",
                    }
                })?;
        }

        if let Some(heading_deg) = packet.state.heading_deg {
            snapshot.kinematics.heading =
                ValidatedAngle::new_degrees(angles::normalize_degrees_signed(heading_deg))
                    .map_err(|_| Ac7TelemetryError::InvalidField {
                        field: "state.heading_deg",
                    })?;
        }

        if let Some(pitch_deg) = packet.state.pitch_deg {
            snapshot.kinematics.pitch = ValidatedAngle::new_degrees(pitch_deg).map_err(|_| {
                Ac7TelemetryError::InvalidField {
                    field: "state.pitch_deg",
                }
            })?;
        }

        if let Some(roll_deg) = packet.state.roll_deg {
            snapshot.kinematics.bank = ValidatedAngle::new_degrees(roll_deg).map_err(|_| {
                Ac7TelemetryError::InvalidField {
                    field: "state.roll_deg",
                }
            })?;
        }

        if let Some(vs_mps) = packet.state.vertical_speed_mps {
            snapshot.kinematics.vertical_speed = conversions::mps_to_fpm(vs_mps);
        }

        if let Some(g_force) = packet.state.g_force {
            snapshot.kinematics.g_force =
                GForce::new(g_force).map_err(|_| Ac7TelemetryError::InvalidField {
                    field: "state.g_force",
                })?;
        }

        if let Some(altitude_m) = packet.state.altitude_m {
            snapshot.environment.altitude = conversions::meters_to_feet(altitude_m);
        }

        if let Some(pitch) = packet.controls.pitch {
            snapshot.control_inputs.pitch = pitch;
        }
        if let Some(roll) = packet.controls.roll {
            snapshot.control_inputs.roll = roll;
        }
        if let Some(yaw) = packet.controls.yaw {
            snapshot.control_inputs.yaw = yaw;
        }
        if let Some(throttle) = packet.controls.throttle {
            snapshot.control_inputs.throttle = vec![throttle];
        }

        snapshot.validity.attitude_valid =
            packet.state.pitch_deg.is_some() && packet.state.roll_deg.is_some();
        snapshot.validity.velocities_valid =
            packet.state.speed_mps.is_some() || packet.state.ground_speed_mps.is_some();
        snapshot.validity.position_valid = packet.state.altitude_m.is_some();
        snapshot.validity.kinematics_valid = packet.state.g_force.is_some();
        snapshot.validity.aero_valid = snapshot.validity.attitude_valid;
        snapshot.validity.safe_for_ffb = snapshot.validity.attitude_valid
            && snapshot.validity.velocities_valid
            && snapshot.validity.position_valid;

        Ok(snapshot)
    }

    /// Returns adapter state.
    pub fn state(&self) -> AdapterState {
        self.state
    }

    /// Returns adapter metrics.
    pub fn metrics(&self) -> AdapterMetrics {
        self.metrics.clone()
    }

    /// Returns shared metrics registry.
    pub fn metrics_registry(&self) -> &MetricsRegistry {
        &self.metrics_registry
    }

    /// Returns mutable access to the bus publisher.
    pub fn bus_publisher_mut(&mut self) -> &mut BusPublisher {
        &mut self.bus_publisher
    }

    /// Returns the local bound address if started.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket.as_ref().and_then(|s| s.local_addr().ok())
    }

    /// Returns source address of the last received packet.
    pub fn source_addr(&self) -> Option<SocketAddr> {
        self.source_addr
    }

    /// Returns true when connection timeout is exceeded.
    pub fn is_connection_timeout(&self) -> bool {
        self.last_packet
            .map(|last| Instant::now().duration_since(last) > self.config.connection_timeout)
            .unwrap_or(true)
    }

    /// Returns time since last packet if available.
    pub fn time_since_last_packet(&self) -> Option<Duration> {
        self.last_packet
            .map(|last| Instant::now().duration_since(last))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_packet() -> Ac7TelemetryPacket {
        Ac7TelemetryPacket::from_json_str(
            &json!({
                "schema": "flight.ac7.telemetry/1",
                "timestamp_ms": 1200,
                "aircraft": "F-16C",
                "state": {
                    "altitude_m": 1000.0,
                    "speed_mps": 200.0,
                    "ground_speed_mps": 195.0,
                    "vertical_speed_mps": 2.0,
                    "heading_deg": 90.0,
                    "pitch_deg": 5.0,
                    "roll_deg": -8.0,
                    "g_force": 1.2
                },
                "controls": {
                    "pitch": 0.1,
                    "roll": -0.2,
                    "yaw": 0.05,
                    "throttle": 0.8
                }
            })
            .to_string(),
        )
        .expect("packet should parse")
    }

    #[test]
    fn converts_packet_to_snapshot() {
        let adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default());
        let snapshot = adapter.convert_packet_to_snapshot(&test_packet()).unwrap();

        assert_eq!(snapshot.sim, SimId::AceCombat7);
        assert_eq!(snapshot.aircraft.icao, "F-16C");
        assert_eq!(snapshot.kinematics.heading.to_degrees(), 90.0);
        assert_eq!(snapshot.control_inputs.throttle, vec![0.8]);
        assert!(snapshot.validity.safe_for_ffb);
    }

    #[tokio::test]
    async fn udp_poll_receives_packet() {
        let mut adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        });
        adapter.start().await.unwrap();
        let target = adapter.local_addr().unwrap();

        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let payload = test_packet().to_json_vec().unwrap();
        sender.send_to(&payload, target).await.unwrap();

        let snapshot = adapter.poll_once().await.unwrap().unwrap();
        assert_eq!(snapshot.sim, SimId::AceCombat7);
        assert_eq!(adapter.state(), AdapterState::Active);
        assert!(adapter.source_addr().is_some());
    }

    #[test]
    fn adapter_initial_state_is_disconnected() {
        let adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default());
        assert_eq!(adapter.state(), AdapterState::Disconnected);
        assert!(adapter.source_addr().is_none());
        assert!(adapter.local_addr().is_none());
        assert!(adapter.is_connection_timeout()); // no packets = timed out
    }

    #[test]
    fn snapshot_has_correct_sim_id() {
        let adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default());
        let snapshot = adapter.convert_packet_to_snapshot(&test_packet()).unwrap();
        assert_eq!(snapshot.sim, SimId::AceCombat7);
    }

    #[test]
    fn snapshot_validity_flags_set_from_full_packet() {
        let adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default());
        let snapshot = adapter.convert_packet_to_snapshot(&test_packet()).unwrap();
        assert!(snapshot.validity.attitude_valid);
        assert!(snapshot.validity.velocities_valid);
        assert!(snapshot.validity.position_valid);
        assert!(snapshot.validity.safe_for_ffb);
    }

    #[test]
    fn snapshot_validity_partial_packet() {
        use flight_ac7_protocol::{Ac7State, Ac7TelemetryPacket};
        let packet = Ac7TelemetryPacket {
            state: Ac7State {
                altitude_m: Some(1000.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default());
        let snapshot = adapter.convert_packet_to_snapshot(&packet).unwrap();
        assert!(snapshot.validity.position_valid);
        assert!(!snapshot.validity.attitude_valid);
        assert!(!snapshot.validity.safe_for_ffb);
    }

    #[test]
    fn snapshot_control_inputs_mapped() {
        let adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default());
        let snapshot = adapter.convert_packet_to_snapshot(&test_packet()).unwrap();
        assert_eq!(snapshot.control_inputs.pitch, 0.1);
        assert_eq!(snapshot.control_inputs.roll, -0.2);
        assert_eq!(snapshot.control_inputs.throttle, vec![0.8]);
    }

    #[test]
    fn metrics_registry_tracks_config() {
        let adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig::default());
        // Verify we can access the registry without panicking
        let _registry = adapter.metrics_registry();
        let _metrics = adapter.metrics();
    }

    #[tokio::test]
    async fn start_stop_cycle() {
        let mut adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        });
        adapter.start().await.unwrap();
        assert_eq!(adapter.state(), AdapterState::Connected);
        assert!(adapter.local_addr().is_some());
        adapter.stop();
        assert_eq!(adapter.state(), AdapterState::Disconnected);
        assert!(adapter.local_addr().is_none());
    }

    #[tokio::test]
    async fn poll_updates_source_addr() {
        let mut adapter = Ac7TelemetryAdapter::new(Ac7TelemetryConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            connection_timeout: std::time::Duration::from_secs(5),
            ..Default::default()
        });
        adapter.start().await.unwrap();
        let target = adapter.local_addr().unwrap();

        let sender = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let payload = test_packet().to_json_vec().unwrap();
        sender.send_to(&payload, target).await.unwrap();

        let snapshot = adapter.poll_once().await.unwrap().unwrap();
        assert!(adapter.source_addr().is_some());
        assert!(adapter.time_since_last_packet().is_some());
        assert!(!adapter.is_connection_timeout());
        let _ = snapshot; // verified in other tests
    }
}
