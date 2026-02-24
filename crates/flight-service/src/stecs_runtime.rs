// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Service-owned VKB STECS runtime ingestion.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flight_hid_support::HidDeviceInfo;
use flight_hid_support::device_support::{
    VkbStecsVariant, vkb_stecs_interface_metadata, vkb_stecs_variant,
};
use flight_hotas_vkb::{
    StecsHealthMonitor, StecsHealthStatus, StecsInputAggregator, StecsInputState,
};
use tokio::sync::{RwLock, oneshot};
use tokio::task::JoinHandle;

use crate::health::HealthStream;

const COMPONENT_NAME: &str = "input_hotas_vkb_stecs";

/// Source abstraction for raw STECS reports.
pub trait VkbStecsReportSource: Send + 'static {
    /// Enumerate visible HID devices.
    fn list_devices(&mut self) -> Vec<HidDeviceInfo>;

    /// Read one raw report for the given interface path.
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
pub struct SimulatedVkbStecsReportSource {
    devices: HashMap<String, SimulatedDeviceState>,
    read_errors: HashMap<String, String>,
}

impl SimulatedVkbStecsReportSource {
    /// Add a simulated HID interface and report sequence.
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

    /// Configure a persistent read error for one interface path.
    pub fn set_read_error(&mut self, device_path: &str, error: impl Into<String>) {
        self.read_errors
            .insert(device_path.to_string(), error.into());
    }
}

impl VkbStecsReportSource for SimulatedVkbStecsReportSource {
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

/// Runtime configuration for STECS ingest worker.
#[derive(Debug, Clone, Copy)]
pub struct VkbStecsRuntimeConfig {
    pub poll_hz: u16,
    /// Strip leading Report ID byte from raw reports.
    pub strip_report_id: bool,
    /// How often to re-enumerate HID devices (in poll ticks).
    /// At 250 Hz a value of 250 means once per second.
    pub discovery_interval_ticks: u32,
}

impl Default for VkbStecsRuntimeConfig {
    fn default() -> Self {
        Self {
            poll_hz: 250,
            strip_report_id: false,
            // Re-enumerate once per second by default.
            discovery_interval_ticks: 250,
        }
    }
}

/// Latest parsed snapshot for one physical STECS throttle.
#[derive(Debug, Clone)]
pub struct VkbStecsSnapshot {
    /// Stable physical identifier (`serial` when available, path stem otherwise).
    pub device_id: String,
    /// STECS variant.
    pub variant: VkbStecsVariant,
    /// Number of HID interfaces discovered for this physical device.
    pub interface_count: u8,
    /// Merged state across virtual controllers.
    pub state: StecsInputState,
    /// Health state for this ingest path.
    pub health: StecsHealthStatus,
    /// Snapshot timestamp in unix epoch milliseconds.
    pub updated_at_epoch_ms: u64,
}

#[derive(Debug)]
struct DeviceRuntimeState {
    variant: VkbStecsVariant,
    aggregator: StecsInputAggregator,
    monitor: StecsHealthMonitor,
}

/// Background ingest runtime for VKB STECS devices.
pub struct VkbStecsInputRuntime {
    snapshots: Arc<RwLock<HashMap<String, VkbStecsSnapshot>>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl VkbStecsInputRuntime {
    /// Start runtime with the provided report source.
    pub fn start(
        mut source: Box<dyn VkbStecsReportSource>,
        health: Arc<HealthStream>,
        config: VkbStecsRuntimeConfig,
    ) -> Self {
        let snapshots = Arc::new(RwLock::new(HashMap::new()));
        let snapshots_worker = Arc::clone(&snapshots);

        let poll_hz = config.poll_hz.max(1) as u64;
        let poll_interval = Duration::from_millis((1000 / poll_hz).max(1));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let worker = tokio::spawn(async move {
            let mut interval = tokio::time::interval(poll_interval);
            let mut states: HashMap<String, DeviceRuntimeState> = HashMap::new();
            // Device list cache — refreshed every discovery_interval_ticks ticks.
            let discovery_interval = config.discovery_interval_ticks.max(1) as u64;
            let mut tick_count: u64 = 0;
            let mut cached_devices: Vec<HidDeviceInfo> = Vec::new();

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        break;
                    }
                    _ = interval.tick() => {
                        // Re-enumerate devices only at the configured cadence.
                        if tick_count.is_multiple_of(discovery_interval) {
                            cached_devices = source.list_devices();
                        }
                        tick_count = tick_count.wrapping_add(1);

                        poll_once(
                            source.as_mut(),
                            &cached_devices,
                            &health,
                            &snapshots_worker,
                            &mut states,
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

    /// Shutdown background worker.
    pub async fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        if let Some(worker) = self.worker.take() {
            let _ = worker.await;
        }
    }

    /// Return a copy of all latest snapshots.
    pub async fn snapshots(&self) -> HashMap<String, VkbStecsSnapshot> {
        self.snapshots.read().await.clone()
    }

    /// Return one snapshot by physical device id.
    pub async fn snapshot(&self, device_id: &str) -> Option<VkbStecsSnapshot> {
        self.snapshots.read().await.get(device_id).cloned()
    }
}

impl Drop for VkbStecsInputRuntime {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn poll_once(
    source: &mut dyn VkbStecsReportSource,
    cached_devices: &[HidDeviceInfo],
    health: &HealthStream,
    snapshots: &Arc<RwLock<HashMap<String, VkbStecsSnapshot>>>,
    states: &mut HashMap<String, DeviceRuntimeState>,
    config: VkbStecsRuntimeConfig,
) {
    let stecs_infos: Vec<HidDeviceInfo> = cached_devices
        .iter()
        .filter(|info| vkb_stecs_variant(info).is_some())
        .cloned()
        .collect();

    let info_by_path: HashMap<String, HidDeviceInfo> = stecs_infos
        .iter()
        .map(|info| (info.device_path.clone(), info.clone()))
        .collect();

    let metadata = vkb_stecs_interface_metadata(stecs_infos.iter());
    let mut grouped: BTreeMap<
        String,
        Vec<flight_hid_support::device_support::VkbStecsInterfaceMetadata>,
    > = BTreeMap::new();
    for interface in metadata {
        grouped
            .entry(interface.physical_id.clone())
            .or_default()
            .push(interface);
    }

    let mut active_ids = HashSet::new();
    for (physical_id, interfaces) in grouped {
        active_ids.insert(physical_id.clone());

        let mut warning_messages: Vec<String> = Vec::new();
        let mut error_messages: Vec<String> = Vec::new();

        let Some(primary_info) = interfaces
            .first()
            .and_then(|interface| info_by_path.get(&interface.device_path))
        else {
            continue;
        };
        let Some(variant) = vkb_stecs_variant(primary_info) else {
            continue;
        };

        let state = states
            .entry(physical_id.clone())
            .or_insert_with(|| DeviceRuntimeState {
                variant,
                aggregator: StecsInputAggregator::new(variant)
                    .with_report_id(config.strip_report_id),
                monitor: StecsHealthMonitor::new(variant),
            });

        if state.variant != variant {
            state.variant = variant;
            state.aggregator =
                StecsInputAggregator::new(variant).with_report_id(config.strip_report_id);
            state.monitor = StecsHealthMonitor::new(variant);
        }

        state.aggregator.begin_poll();

        let mut cycle_successes = 0u32;
        let mut cycle_failures = 0u32;

        for interface in &interfaces {
            match source.read_report(&interface.device_path) {
                Ok(Some(report)) => match state
                    .aggregator
                    .merge_interface_report(interface.virtual_controller_index, &report)
                {
                    Ok(()) => {
                        cycle_successes = cycle_successes.saturating_add(1);
                    }
                    Err(error) => {
                        cycle_failures = cycle_failures.saturating_add(1);
                        error_messages.push(format!(
                            "{} interface {} parse failure: {}",
                            physical_id, interface.device_path, error
                        ));
                    }
                },
                Ok(None) => {}
                Err(error) => {
                    cycle_failures = cycle_failures.saturating_add(1);
                    error_messages.push(format!(
                        "{} interface {} read failure: {}",
                        physical_id, interface.device_path, error
                    ));
                }
            }
        }

        if cycle_successes > 0 {
            state.monitor.record_success();
            let merged_state = state.aggregator.snapshot();
            let active_vc_count = merged_state
                .active_virtual_controllers
                .iter()
                .filter(|active| **active)
                .count() as u8;
            let interface_count = u8::try_from(interfaces.len()).unwrap_or(u8::MAX);
            let health_status = state.monitor.status(true, interface_count, active_vc_count);
            let snapshot = VkbStecsSnapshot {
                device_id: physical_id.clone(),
                variant,
                interface_count,
                state: merged_state,
                health: health_status,
                updated_at_epoch_ms: unix_epoch_ms_now(),
            };
            snapshots
                .write()
                .await
                .insert(physical_id.clone(), snapshot);
        } else if cycle_failures > 0 {
            let threshold_reached = state.monitor.record_failure();
            if threshold_reached {
                warning_messages.push(format!(
                    "{} read/parse failures exceeded threshold",
                    physical_id
                ));
            }
        } else if state.monitor.should_check_health() {
            let interface_count = u8::try_from(interfaces.len()).unwrap_or(u8::MAX);
            let status = state.monitor.status(true, interface_count, 0);
            if !status.is_healthy() {
                warning_messages.push(format!(
                    "{} health degraded: failures={}",
                    physical_id, status.consecutive_failures
                ));
            }
            state.monitor.mark_health_checked();
        }

        for message in warning_messages {
            health.warning(COMPONENT_NAME, &message).await;
        }
        for message in error_messages {
            health.error(COMPONENT_NAME, &message, None).await;
        }
    }

    let removed_ids: Vec<String> = states
        .keys()
        .filter(|id| !active_ids.contains(*id))
        .cloned()
        .collect();
    for removed in removed_ids {
        states.remove(&removed);
        snapshots.write().await.remove(&removed);
        health
            .warning(
                COMPONENT_NAME,
                &format!("{removed} disconnected from runtime"),
            )
            .await;
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

    use super::*;

    fn stecs_device_info(
        product_id: u16,
        serial: &str,
        path: &str,
        interface_number: u16,
    ) -> HidDeviceInfo {
        HidDeviceInfo {
            vendor_id: flight_hid_support::device_support::VKB_VENDOR_ID,
            product_id,
            serial_number: Some(serial.to_string()),
            manufacturer: Some("VKB".to_string()),
            product_name: Some("VKB STECS".to_string()),
            device_path: path.to_string(),
            usage_page: flight_hid_support::device_support::USAGE_PAGE_GENERIC_DESKTOP,
            usage: if interface_number == 0 {
                flight_hid_support::device_support::USAGE_JOYSTICK
            } else {
                0
            },
            report_descriptor: None,
        }
    }

    async fn wait_for_snapshot_count(
        runtime: &VkbStecsInputRuntime,
        expected: usize,
    ) -> HashMap<String, VkbStecsSnapshot> {
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
    async fn test_runtime_merges_virtual_controller_button_ranges() {
        let health = Arc::new(HealthStream::new());
        let mut source = SimulatedVkbStecsReportSource::default();

        let vc0_path = r"\\?\hid#vid_231d&pid_013c&mi_00#7#if0";
        let vc1_path = r"\\?\hid#vid_231d&pid_013c&mi_01#7#if1";

        source.add_device(
            stecs_device_info(
                flight_hid_support::device_support::VKB_STECS_RIGHT_SPACE_STANDARD_PID,
                "STECS123",
                vc0_path,
                0,
            ),
            vec![vec![
                0x00, 0x00, // rx
                0x00, 0x00, // ry
                0x00, 0x00, // x
                0x00, 0x00, // y
                0x00, 0x00, // z
                0x01, 0x00, 0x00, 0x00, // button 1
            ]],
        );
        source.add_device(
            stecs_device_info(
                flight_hid_support::device_support::VKB_STECS_RIGHT_SPACE_STANDARD_PID,
                "STECS123",
                vc1_path,
                1,
            ),
            vec![vec![0x01, 0x00, 0x00, 0x00]], // button 33
        );

        let mut runtime = VkbStecsInputRuntime::start(
            Box::new(source),
            health,
            VkbStecsRuntimeConfig {
                poll_hz: 120,
                strip_report_id: false,
                discovery_interval_ticks: 1,
            },
        );

        let snapshots = wait_for_snapshot_count(&runtime, 1).await;
        assert_eq!(snapshots.len(), 1);

        let snapshot = snapshots.values().next().expect("snapshot");
        assert_eq!(
            snapshot.variant,
            VkbStecsVariant::RightSpaceThrottleGripStandard
        );
        assert_eq!(snapshot.interface_count, 2);
        assert!(snapshot.state.buttons[0]);
        assert!(snapshot.state.buttons[32]);
        assert!(snapshot.health.is_healthy());

        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn test_runtime_read_failure_emits_health_event() {
        let health = Arc::new(HealthStream::new());
        let mut events = health.subscribe();
        let mut source = SimulatedVkbStecsReportSource::default();
        let path = "/dev/vkb-stecs-vc0";

        source.add_device(
            stecs_device_info(
                flight_hid_support::device_support::VKB_STECS_LEFT_SPACE_MINI_PLUS_PID,
                "STECSERR",
                path,
                0,
            ),
            vec![vec![0x01, 0x00, 0x00, 0x00]],
        );
        source.set_read_error(path, "simulated io failure");

        let mut runtime = VkbStecsInputRuntime::start(
            Box::new(source),
            Arc::clone(&health),
            VkbStecsRuntimeConfig {
                poll_hz: 80,
                strip_report_id: false,
                discovery_interval_ticks: 1,
            },
        );

        let mut saw_error = false;
        for _ in 0..20 {
            if let Ok(event) = tokio::time::timeout(Duration::from_millis(120), events.recv()).await
                && let Ok(event) = event
                && event.message.contains("read failure")
            {
                saw_error = true;
                break;
            }
        }

        assert!(saw_error);
        runtime.shutdown().await;
    }
}
