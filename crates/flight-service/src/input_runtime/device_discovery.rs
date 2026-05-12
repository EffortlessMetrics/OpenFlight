// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Device discovery and runtime state lifecycle for T.Flight ingest.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use flight_hid_support::device_support::{
    axis_mode_from_device_info, axis_mode_warning, driver_note, pc_mode_note,
};
use flight_hotas_thrustmaster::{
    AxisMode, TFlightHealthMonitor, TFlightInputHandler, is_hotas4_legacy_pid, tflight_model,
};
use tokio::sync::RwLock;

use super::{
    COMPONENT_NAME, DeviceRuntimeState, TFlightReportSource, TFlightRuntimeConfig, TFlightSnapshot,
};
use crate::health::HealthStream;

pub(super) async fn refresh_devices(
    source: &mut dyn TFlightReportSource,
    health: &HealthStream,
    states: &mut HashMap<String, DeviceRuntimeState>,
    config: TFlightRuntimeConfig,
) -> HashSet<String> {
    let mut active_paths = HashSet::new();

    for info in source.list_devices() {
        let Some(model) = tflight_model(&info) else {
            continue;
        };

        let path = info.device_path.clone();
        active_paths.insert(path.clone());

        if let Some(existing) = states.get_mut(&path) {
            let axis_mode_hint = axis_mode_from_device_info(&info);
            existing.info = info;
            existing.handler.set_yaw_policy(config.yaw_policy);
            existing.handler.set_axis_mode_hint(axis_mode_hint);
            continue;
        }

        let axis_mode_hint = axis_mode_from_device_info(&info);
        let has_descriptor = info.report_descriptor.is_some();
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
            .with_report_id(config.strip_report_id)
            .with_axis_mode_hint(axis_mode_hint);
        let monitor = TFlightHealthMonitor::new(model).with_legacy_pid(is_legacy);

        states.insert(
            path.clone(),
            DeviceRuntimeState {
                info,
                snapshot_key: snapshot_key.clone(),
                model,
                handler,
                monitor,
                last_mode: AxisMode::Unknown,
                ghost_warning_active: false,
                merged_reports_streak: 0,
                merged_mode_guidance_emitted: false,
                is_legacy_pid: is_legacy,
            },
        );

        emit_new_device_health(
            health,
            &snapshot_key,
            model,
            axis_mode_hint,
            has_descriptor,
            is_legacy,
        )
        .await;
    }

    active_paths
}

pub(super) async fn remove_disconnected_devices(
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

async fn emit_new_device_health(
    health: &HealthStream,
    snapshot_key: &str,
    model: flight_hotas_thrustmaster::TFlightModel,
    axis_mode_hint: AxisMode,
    has_descriptor: bool,
    is_legacy: bool,
) {
    if is_legacy {
        health
            .info(
                COMPONENT_NAME,
                &format!("{} detected via HOTAS 4 legacy PID", snapshot_key),
            )
            .await;
    }

    if axis_mode_hint != AxisMode::Unknown {
        health
            .info(
                COMPONENT_NAME,
                &format!(
                    "{} descriptor advertises {} axis layout (auto-detection still active)",
                    snapshot_key,
                    axis_mode_hint.as_str()
                ),
            )
            .await;
    }

    if let Some(mode_warning) = axis_mode_warning(axis_mode_hint) {
        health
            .warning(
                COMPONENT_NAME,
                &format!(
                    "{} descriptor hint indicates merged layout. {} {} {}",
                    snapshot_key,
                    mode_warning,
                    pc_mode_note(model),
                    driver_note()
                ),
            )
            .await;
    } else if axis_mode_hint == AxisMode::Unknown && has_descriptor {
        health
            .warning(
                COMPONENT_NAME,
                &format!(
                    "{} descriptor axis layout is ambiguous; runtime will rely on report-length detection. {} {}",
                    snapshot_key,
                    pc_mode_note(model),
                    driver_note()
                ),
            )
            .await;
    }
}
