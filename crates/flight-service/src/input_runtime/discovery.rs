// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use flight_hid_support::HidDeviceInfo;
use flight_hid_support::device_support::{
    axis_mode_from_device_info, axis_mode_warning, driver_note, pc_mode_note,
};
use flight_hotas_thrustmaster::{
    AxisMode, TFlightHealthMonitor, TFlightInputHandler, TFlightModel, is_hotas4_legacy_pid,
    tflight_model,
};
use tokio::sync::RwLock;

use crate::health::HealthStream;

use super::{
    COMPONENT_NAME, DeviceRuntimeState, TFlightReportSource, TFlightRuntimeConfig, TFlightSnapshot,
};

pub(super) async fn reconcile_devices(
    source: &mut dyn TFlightReportSource,
    health: &HealthStream,
    snapshots: &Arc<RwLock<HashMap<String, TFlightSnapshot>>>,
    states: &mut HashMap<String, DeviceRuntimeState>,
    config: TFlightRuntimeConfig,
) {
    let mut active_paths = HashSet::new();

    for info in source.list_devices() {
        let Some(device) = DiscoveredTFlightDevice::from_info(info) else {
            continue;
        };
        active_paths.insert(device.path.clone());

        if refresh_existing_state(states, &device, config) {
            continue;
        }

        insert_new_state(states, &device, config);
        emit_new_device_guidance(health, &device).await;
    }

    remove_inactive_devices(health, snapshots, states, &active_paths).await;
}

struct DiscoveredTFlightDevice {
    info: HidDeviceInfo,
    path: String,
    snapshot_key: String,
    model: TFlightModel,
    axis_mode_hint: AxisMode,
    has_descriptor: bool,
    is_legacy_pid: bool,
}

impl DiscoveredTFlightDevice {
    fn from_info(info: HidDeviceInfo) -> Option<Self> {
        let model = tflight_model(&info)?;
        let path = info.device_path.clone();
        let snapshot_key = info
            .serial_number
            .clone()
            .unwrap_or_else(|| info.device_path.clone());
        let axis_mode_hint = axis_mode_from_device_info(&info);
        let has_descriptor = info.report_descriptor.is_some();
        let is_legacy_pid = is_hotas4_legacy_pid(&info);

        Some(Self {
            info,
            path,
            snapshot_key,
            model,
            axis_mode_hint,
            has_descriptor,
            is_legacy_pid,
        })
    }
}

fn refresh_existing_state(
    states: &mut HashMap<String, DeviceRuntimeState>,
    device: &DiscoveredTFlightDevice,
    config: TFlightRuntimeConfig,
) -> bool {
    let Some(existing) = states.get_mut(&device.path) else {
        return false;
    };

    existing.info = device.info.clone();
    existing.handler.set_yaw_policy(config.yaw_policy);
    existing.handler.set_axis_mode_hint(device.axis_mode_hint);
    true
}

fn insert_new_state(
    states: &mut HashMap<String, DeviceRuntimeState>,
    device: &DiscoveredTFlightDevice,
    config: TFlightRuntimeConfig,
) {
    // Always start in Unknown so the handler auto-detects every report;
    // the descriptor hint is advisory only (see fix for runtime AxisMode pinning).
    let handler = TFlightInputHandler::with_axis_mode(device.model, AxisMode::Unknown)
        .with_yaw_policy(config.yaw_policy)
        .with_throttle_inversion(config.throttle_inversion)
        .with_report_id(config.strip_report_id)
        .with_axis_mode_hint(device.axis_mode_hint);
    let monitor = TFlightHealthMonitor::new(device.model).with_legacy_pid(device.is_legacy_pid);

    states.insert(
        device.path.clone(),
        DeviceRuntimeState {
            info: device.info.clone(),
            snapshot_key: device.snapshot_key.clone(),
            model: device.model,
            handler,
            monitor,
            last_mode: AxisMode::Unknown,
            ghost_warning_active: false,
            merged_reports_streak: 0,
            merged_mode_guidance_emitted: false,
            is_legacy_pid: device.is_legacy_pid,
        },
    );
}

async fn emit_new_device_guidance(health: &HealthStream, device: &DiscoveredTFlightDevice) {
    if device.is_legacy_pid {
        health
            .info(
                COMPONENT_NAME,
                &format!("{} detected via HOTAS 4 legacy PID", device.snapshot_key),
            )
            .await;
    }

    if device.axis_mode_hint != AxisMode::Unknown {
        health
            .info(
                COMPONENT_NAME,
                &format!(
                    "{} descriptor advertises {} axis layout (auto-detection still active)",
                    device.snapshot_key,
                    device.axis_mode_hint.as_str()
                ),
            )
            .await;
    }

    if let Some(mode_warning) = axis_mode_warning(device.axis_mode_hint) {
        health
            .warning(
                COMPONENT_NAME,
                &format!(
                    "{} descriptor hint indicates merged layout. {} {} {}",
                    device.snapshot_key,
                    mode_warning,
                    pc_mode_note(device.model),
                    driver_note()
                ),
            )
            .await;
    } else if device.axis_mode_hint == AxisMode::Unknown && device.has_descriptor {
        health
            .warning(
                COMPONENT_NAME,
                &format!(
                    "{} descriptor axis layout is ambiguous; runtime will rely on report-length detection. {} {}",
                    device.snapshot_key,
                    pc_mode_note(device.model),
                    driver_note()
                ),
            )
            .await;
    }
}

async fn remove_inactive_devices(
    health: &HealthStream,
    snapshots: &Arc<RwLock<HashMap<String, TFlightSnapshot>>>,
    states: &mut HashMap<String, DeviceRuntimeState>,
    active_paths: &HashSet<String>,
) {
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
}
