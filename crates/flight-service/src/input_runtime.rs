// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Service-owned T.Flight HOTAS runtime ingestion.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flight_hid_support::HidDeviceInfo;
use flight_hid_support::ghost_filter::GhostFilterStats;
use flight_hotas_thrustmaster::{
    AxisMode, TFlightHealthMonitor, TFlightHealthStatus, TFlightInputHandler, TFlightInputState,
    TFlightModel, TFlightYawPolicy, TFlightYawResolution, is_hotas4_legacy_pid, tflight_model,
};
use tokio::sync::{RwLock, oneshot};
use tokio::task::JoinHandle;

use crate::health::HealthStream;

const COMPONENT_NAME: &str = "input_hotas_tflight";
const GHOST_WARNING_THRESHOLD: f64 = 0.05;

/// Source abstraction for raw T.Flight reports.
pub trait TFlightReportSource: Send + 'static {
    /// Enumerate visible HID devices.
    fn list_devices(&mut self) -> Vec<HidDeviceInfo>;

    /// Read one raw report for the given device path.
    fn read_report(&mut self, device_path: &str) -> Result<Option<Vec<u8>>, String>;
}

#[derive(Debug, Clone)]
struct SimulatedDeviceState {
    info: HidDeviceInfo,
    reports: Vec<Vec<u8>>,
    next_report: usize,
}

/// Deterministic simulated source used by tests and non-hardware runtime mode.
#[derive(Debug, Default)]
pub struct SimulatedTFlightReportSource {
    devices: HashMap<String, SimulatedDeviceState>,
    read_errors: HashMap<String, String>,
}

impl SimulatedTFlightReportSource {
    /// Add a simulated device and report sequence.
    pub fn add_device(&mut self, info: HidDeviceInfo, reports: Vec<Vec<u8>>) {
        self.devices.insert(
            info.device_path.clone(),
            SimulatedDeviceState {
                info,
                reports,
                next_report: 0,
            },
        );
    }

    /// Configure a persistent read error for a simulated device.
    pub fn set_read_error(&mut self, device_path: &str, error: impl Into<String>) {
        self.read_errors
            .insert(device_path.to_string(), error.into());
    }
}

impl TFlightReportSource for SimulatedTFlightReportSource {
    fn list_devices(&mut self) -> Vec<HidDeviceInfo> {
        self.devices
            .values()
            .map(|state| state.info.clone())
            .collect()
    }

    fn read_report(&mut self, device_path: &str) -> Result<Option<Vec<u8>>, String> {
        if let Some(error) = self.read_errors.get(device_path) {
            return Err(error.clone());
        }

        let Some(state) = self.devices.get_mut(device_path) else {
            return Ok(None);
        };

        if state.reports.is_empty() {
            return Ok(None);
        }

        let index = state.next_report % state.reports.len();
        state.next_report = state.next_report.wrapping_add(1);
        Ok(Some(state.reports[index].clone()))
    }
}

/// Runtime configuration for T.Flight ingest worker.
#[derive(Debug, Clone, Copy)]
pub struct TFlightRuntimeConfig {
    pub poll_hz: u16,
    pub yaw_policy: TFlightYawPolicy,
    /// Apply throttle axis inversion. Disabled by default; enable per profile
    /// only after hardware receipts confirm the inversion is needed.
    pub throttle_inversion: bool,
    /// Strip leading Report ID byte from raw reports. Disabled by default.
    /// Enable when the OS/driver stack prepends a Report ID before the payload.
    pub strip_report_id: bool,
}

impl Default for TFlightRuntimeConfig {
    fn default() -> Self {
        Self {
            poll_hz: 250,
            yaw_policy: TFlightYawPolicy::Auto,
            throttle_inversion: false,
            strip_report_id: false,
        }
    }
}

/// Latest parsed device snapshot with health metadata.
#[derive(Debug, Clone)]
pub struct TFlightSnapshot {
    pub device_id: String,
    pub device_path: String,
    pub model: TFlightModel,
    pub axis_mode: AxisMode,
    pub state: TFlightInputState,
    pub yaw: TFlightYawResolution,
    pub ghost_rate: f64,
    pub ghost_stats: GhostFilterStats,
    pub health: TFlightHealthStatus,
    pub is_legacy_pid: bool,
    pub updated_at_epoch_ms: u64,
}

#[derive(Debug)]
struct DeviceRuntimeState {
    info: HidDeviceInfo,
    snapshot_key: String,
    handler: TFlightInputHandler,
    monitor: TFlightHealthMonitor,
    last_mode: AxisMode,
    ghost_warning_active: bool,
    is_legacy_pid: bool,
}

/// Background ingest runtime for T.Flight devices.
pub struct TFlightInputRuntime {
    snapshots: Arc<RwLock<HashMap<String, TFlightSnapshot>>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl TFlightInputRuntime {
    /// Start runtime with the provided report source.
    pub fn start(
        mut source: Box<dyn TFlightReportSource>,
        health: Arc<HealthStream>,
        config: TFlightRuntimeConfig,
    ) -> Self {
        let snapshots = Arc::new(RwLock::new(HashMap::new()));
        let snapshots_worker = Arc::clone(&snapshots);

        let poll_hz = config.poll_hz.max(1) as u64;
        let poll_interval = Duration::from_millis((1000 / poll_hz).max(1));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let worker = tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval);
            let mut device_states: HashMap<String, DeviceRuntimeState> = HashMap::new();

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        break;
                    }
                    _ = interval.tick() => {
                        poll_once(
                            source.as_mut(),
                            &health,
                            &snapshots_worker,
                            &mut device_states,
                            config,
                        ).await;
                    }
                }
            }
        });

        Self {
            snapshots,
            shutdown_tx: Some(shutdown_tx),
            worker: Some(worker),
        }
    }

    /// Shutdown the background worker.
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(worker) = self.worker.take() {
            let _ = worker.await;
        }
    }

    /// Return a copy of all latest snapshots.
    pub async fn snapshots(&self) -> HashMap<String, TFlightSnapshot> {
        self.snapshots.read().await.clone()
    }

    /// Return a copy of one snapshot by device id.
    pub async fn snapshot(&self, device_id: &str) -> Option<TFlightSnapshot> {
        self.snapshots.read().await.get(device_id).cloned()
    }
}

impl Drop for TFlightInputRuntime {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn poll_once(
    source: &mut dyn TFlightReportSource,
    health: &HealthStream,
    snapshots: &Arc<RwLock<HashMap<String, TFlightSnapshot>>>,
    states: &mut HashMap<String, DeviceRuntimeState>,
    config: TFlightRuntimeConfig,
) {
    let mut active_paths = HashSet::new();

    for info in source.list_devices() {
        let Some(model) = tflight_model(&info) else {
            continue;
        };

        let path = info.device_path.clone();
        active_paths.insert(path.clone());

        if let Some(existing) = states.get_mut(&path) {
            existing.info = info;
            existing.handler.set_yaw_policy(config.yaw_policy);
            continue;
        }

        let _axis_mode_hint = flight_hid_support::device_support::axis_mode_from_device_info(&info);
        let is_legacy = is_hotas4_legacy_pid(&info);
        let snapshot_key = info
            .serial_number
            .clone()
            .unwrap_or_else(|| info.device_path.clone());
        // Always start in Unknown so the handler auto-detects every report;
        // the descriptor hint is advisory only (see fix for runtime AxisMode pinning).
        let handler = TFlightInputHandler::with_axis_mode(model, AxisMode::Unknown)
            .with_yaw_policy(config.yaw_policy)
            .with_throttle_inversion(config.throttle_inversion)
            .with_report_id(config.strip_report_id);
        let monitor = TFlightHealthMonitor::new(model).with_legacy_pid(is_legacy);

        states.insert(
            path.clone(),
            DeviceRuntimeState {
                info,
                snapshot_key: snapshot_key.clone(),
                handler,
                monitor,
                last_mode: AxisMode::Unknown,
                ghost_warning_active: false,
                is_legacy_pid: is_legacy,
            },
        );

        if is_legacy {
            health
                .info(
                    COMPONENT_NAME,
                    &format!("{} detected via HOTAS 4 legacy PID", snapshot_key),
                )
                .await;
        }

        if _axis_mode_hint != AxisMode::Unknown {
            health
                .info(
                    COMPONENT_NAME,
                    &format!(
                        "{} descriptor advertises {} axis layout (auto-detection still active)",
                        snapshot_key,
                        _axis_mode_hint.as_str()
                    ),
                )
                .await;
        }
    }

    let removed_paths: Vec<String> = states
        .keys()
        .filter(|path| !active_paths.contains(*path))
        .cloned()
        .collect();

    for path in removed_paths {
        if let Some(removed) = states.remove(&path) {
            snapshots.write().await.remove(&removed.snapshot_key);
            health
                .warning(
                    COMPONENT_NAME,
                    &format!("{} disconnected from runtime", removed.snapshot_key),
                )
                .await;
        }
    }

    let paths: Vec<String> = states.keys().cloned().collect();
    for path in paths {
        let read_result = source.read_report(&path);

        let mut info_messages: Vec<String> = Vec::new();
        let mut warning_messages: Vec<String> = Vec::new();
        let mut error_messages: Vec<String> = Vec::new();
        let mut snapshot_update: Option<(String, TFlightSnapshot)> = None;

        {
            let Some(state) = states.get_mut(&path) else {
                continue;
            };

            match read_result {
                Ok(Some(report)) => match state.handler.try_parse_report(&report) {
                    Ok(parsed) => {
                        state.monitor.record_success();

                        let current_mode = state.handler.current_axis_mode();
                        if current_mode != state.last_mode {
                            info_messages.push(format!(
                                "{} axis mode changed: {} -> {}",
                                state.snapshot_key,
                                state.last_mode.as_str(),
                                current_mode.as_str()
                            ));
                            state.last_mode = current_mode;
                        }

                        let yaw = state.handler.resolve_yaw(&parsed);
                        let ghost_rate = state.handler.ghost_rate();
                        let ghost_stats = state.handler.ghost_stats();

                        if ghost_rate > GHOST_WARNING_THRESHOLD && !state.ghost_warning_active {
                            warning_messages.push(format!(
                                "{} ghost input rate high: {:.2}%",
                                state.snapshot_key,
                                ghost_rate * 100.0
                            ));
                            state.ghost_warning_active = true;
                        } else if ghost_rate <= (GHOST_WARNING_THRESHOLD * 0.5) {
                            state.ghost_warning_active = false;
                        }

                        let health_status =
                            state.monitor.status(true, ghost_rate, ghost_stats.clone());
                        let snapshot = TFlightSnapshot {
                            device_id: state.snapshot_key.clone(),
                            device_path: state.info.device_path.clone(),
                            model: health_status.device_type,
                            axis_mode: current_mode,
                            state: parsed,
                            yaw,
                            ghost_rate,
                            ghost_stats,
                            health: health_status,
                            is_legacy_pid: state.is_legacy_pid,
                            updated_at_epoch_ms: unix_epoch_ms_now(),
                        };

                        snapshot_update = Some((state.snapshot_key.clone(), snapshot));
                    }
                    Err(error) => {
                        let threshold_reached = state.monitor.record_failure();
                        error_messages.push(format!(
                            "{} report parse failure: {}",
                            state.snapshot_key, error
                        ));
                        if threshold_reached {
                            error_messages.push(format!(
                                "{} report parse failures exceeded threshold",
                                state.snapshot_key
                            ));
                        }
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    let threshold_reached = state.monitor.record_failure();
                    error_messages.push(format!("{} read failure: {}", state.snapshot_key, error));
                    if threshold_reached {
                        error_messages.push(format!(
                            "{} read failures exceeded threshold",
                            state.snapshot_key
                        ));
                    }
                }
            }
        }

        if let Some((snapshot_key, snapshot)) = snapshot_update {
            snapshots.write().await.insert(snapshot_key, snapshot);
        }

        for message in info_messages {
            health.info(COMPONENT_NAME, &message).await;
        }
        for message in warning_messages {
            health.warning(COMPONENT_NAME, &message).await;
        }
        for message in error_messages {
            health.error(COMPONENT_NAME, &message, None).await;
        }
    }
}

fn unix_epoch_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

    fn hotas4_device_info(product_id: u16, device_path: &str) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: flight_hid_support::device_support::THRUSTMASTER_VENDOR_ID,
            product_id,
            serial_number: None,
            manufacturer: Some("Thrustmaster".to_string()),
            product_name: Some("T.Flight HOTAS 4".to_string()),
            device_path: device_path.to_string(),
            usage_page: flight_hid_support::device_support::USAGE_PAGE_GENERIC_DESKTOP,
            usage: flight_hid_support::device_support::USAGE_JOYSTICK,
            report_descriptor: None,
        }
    }

    async fn wait_for_snapshot_count(
        runtime: &TFlightInputRuntime,
        expected: usize,
    ) -> HashMap<String, TFlightSnapshot> {
        for _ in 0..40 {
            let snapshots = runtime.snapshots().await;
            if snapshots.len() >= expected {
                return snapshots;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        runtime.snapshots().await
    }

    #[tokio::test]
    async fn test_runtime_ingests_reports_and_updates_cache() {
        let health = Arc::new(HealthStream::new());
        let mut source = SimulatedTFlightReportSource::default();

        source.add_device(
            hotas4_device_info(
                flight_hid_support::device_support::TFLIGHT_HOTAS_4_PID,
                "/dev/hotas4-0",
            ),
            vec![vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x40, 0xC0, 0x00, 0x00]],
        );

        let mut runtime = TFlightInputRuntime::start(
            Box::new(source),
            health,
            TFlightRuntimeConfig {
                poll_hz: 100,
                yaw_policy: TFlightYawPolicy::Auto,
                throttle_inversion: false,
                strip_report_id: false,
            },
        );

        let snapshots = wait_for_snapshot_count(&runtime, 1).await;
        assert_eq!(snapshots.len(), 1);

        let snapshot = snapshots.values().next().unwrap();
        assert_eq!(snapshot.axis_mode, AxisMode::Separate);
        assert_eq!(
            snapshot.yaw.source,
            flight_hotas_thrustmaster::TFlightYawSource::Aux
        );
        assert!(snapshot.updated_at_epoch_ms > 0);

        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_mode_change_emits_health_event() {
        let health = Arc::new(HealthStream::new());
        let mut events = health.subscribe();

        let mut source = SimulatedTFlightReportSource::default();
        source.add_device(
            hotas4_device_info(
                flight_hid_support::device_support::TFLIGHT_HOTAS_4_PID,
                "/dev/hotas4-1",
            ),
            vec![
                vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00],
                vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0xFF, 0x00, 0x00],
            ],
        );

        let mut runtime = TFlightInputRuntime::start(
            Box::new(source),
            Arc::clone(&health),
            TFlightRuntimeConfig {
                poll_hz: 120,
                yaw_policy: TFlightYawPolicy::Auto,
                throttle_inversion: false,
                strip_report_id: false,
            },
        );

        let mut saw_mode_change = false;
        for _ in 0..20 {
            let event = timeout(Duration::from_millis(120), events.recv()).await;
            if let Ok(Ok(event)) = event
                && event.message.contains("axis mode changed")
            {
                saw_mode_change = true;
                break;
            }
        }

        assert!(saw_mode_change);
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_ghost_warning_emits_health_event() {
        let health = Arc::new(HealthStream::new());
        let mut events = health.subscribe();

        let mut source = SimulatedTFlightReportSource::default();
        // Buttons 0 and 2 set (0b0101) trigger impossible-state filter for tflight preset.
        source.add_device(
            hotas4_device_info(
                flight_hid_support::device_support::TFLIGHT_HOTAS_4_PID,
                "/dev/hotas4-2",
            ),
            vec![vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x05, 0x00]],
        );

        let mut runtime = TFlightInputRuntime::start(
            Box::new(source),
            Arc::clone(&health),
            TFlightRuntimeConfig {
                poll_hz: 80,
                yaw_policy: TFlightYawPolicy::Auto,
                throttle_inversion: false,
                strip_report_id: false,
            },
        );

        let mut saw_ghost_warning = false;
        for _ in 0..20 {
            let event = timeout(Duration::from_millis(150), events.recv()).await;
            if let Ok(Ok(event)) = event
                && event.message.contains("ghost input rate high")
            {
                saw_ghost_warning = true;
                break;
            }
        }

        assert!(saw_ghost_warning);
        runtime.shutdown().await;
    }

    /// AC-16.1 — runtime handler always starts Unknown so reports are auto-detected.
    ///
    /// Feeds an 8-byte merged report; the snapshot must reflect `Merged`.
    /// Before the AxisMode-pinning fix this test would fail if a descriptor
    /// indicated `Separate` (handler would reject the shorter report).
    #[tokio::test]
    async fn test_runtime_auto_detects_axis_mode_merged() {
        let health = Arc::new(HealthStream::new());
        let mut source = SimulatedTFlightReportSource::default();

        source.add_device(
            hotas4_device_info(
                flight_hid_support::device_support::TFLIGHT_HOTAS_4_PID,
                "/dev/hotas4-merged-auto",
            ),
            vec![vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00]], // 8-byte merged
        );

        let mut runtime = TFlightInputRuntime::start(
            Box::new(source),
            health,
            TFlightRuntimeConfig {
                poll_hz: 100,
                yaw_policy: TFlightYawPolicy::Auto,
                throttle_inversion: false,
                strip_report_id: false,
            },
        );

        let snapshots = wait_for_snapshot_count(&runtime, 1).await;
        assert_eq!(snapshots.len(), 1);
        let snapshot = snapshots.values().next().unwrap();
        assert_eq!(snapshot.axis_mode, AxisMode::Merged);

        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_legacy_pid_propagates_to_snapshot() {
        let health = Arc::new(HealthStream::new());
        let mut source = SimulatedTFlightReportSource::default();

        source.add_device(
            hotas4_device_info(
                flight_hid_support::device_support::TFLIGHT_HOTAS_4_PID_LEGACY,
                "/dev/hotas4-legacy",
            ),
            vec![vec![0x00, 0x80, 0x00, 0x80, 0x80, 0x80, 0x00, 0x00]],
        );

        let mut runtime = TFlightInputRuntime::start(
            Box::new(source),
            Arc::clone(&health),
            TFlightRuntimeConfig::default(),
        );

        let snapshots = wait_for_snapshot_count(&runtime, 1).await;
        assert_eq!(snapshots.len(), 1);
        let snapshot = snapshots.values().next().unwrap();
        assert!(snapshot.is_legacy_pid);
        assert!(snapshot.health.is_legacy_pid);

        runtime.shutdown().await;
    }

    /// AC-16.4 — throttle inversion is applied when `TFlightRuntimeConfig::throttle_inversion` is true.
    ///
    /// Uses a merged report with throttle byte = 0x00 (raw "fully pushed away").
    /// Without inversion this maps to approximately -1.0 (min).
    /// With inversion it should map to approximately +1.0 (max).
    #[tokio::test]
    async fn test_runtime_throttle_inversion_applied() {
        async fn throttle_for_config(inversion: bool) -> f32 {
            let health = Arc::new(HealthStream::new());
            let mut source = SimulatedTFlightReportSource::default();
            let path = if inversion {
                "/dev/hotas4-inv-on"
            } else {
                "/dev/hotas4-inv-off"
            };
            // 8-byte merged report; throttle byte (index 4) = 0x00
            source.add_device(
                HidDeviceInfo {
                    vendor_id: flight_hid_support::device_support::THRUSTMASTER_VENDOR_ID,
                    product_id: flight_hid_support::device_support::TFLIGHT_HOTAS_4_PID,
                    serial_number: None,
                    manufacturer: None,
                    product_name: None,
                    device_path: path.to_string(),
                    usage_page: flight_hid_support::device_support::USAGE_PAGE_GENERIC_DESKTOP,
                    usage: flight_hid_support::device_support::USAGE_JOYSTICK,
                    report_descriptor: None,
                },
                vec![vec![0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00]],
            );
            let mut runtime = TFlightInputRuntime::start(
                Box::new(source),
                health,
                TFlightRuntimeConfig {
                    poll_hz: 100,
                    yaw_policy: TFlightYawPolicy::Auto,
                    throttle_inversion: inversion,
                    strip_report_id: false,
                },
            );
            let snapshots = wait_for_snapshot_count(&runtime, 1).await;
            let throttle = snapshots
                .values()
                .next()
                .map(|s| s.state.axes.throttle)
                .unwrap_or(0.0);
            runtime.shutdown().await;
            throttle
        }

        let no_inv = throttle_for_config(false).await;
        let with_inv = throttle_for_config(true).await;
        // Throttle byte 0x00 → 0.0 without inversion, 1.0 with inversion (1 - value).
        assert!((no_inv - 0.0).abs() < 0.05, "raw throttle min should be ~0.0");
        assert!((with_inv - 1.0).abs() < 0.05, "inverted throttle min should be ~1.0");
    }
}
