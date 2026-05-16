// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use std::collections::HashMap;
use std::sync::Arc;

use flight_hotas_thrustmaster::{AxisMode, TFlightInputState};
use tokio::sync::RwLock;

use crate::health::HealthStream;

use super::health_messages::HealthMessages;
use super::{
    DeviceRuntimeState, GHOST_WARNING_THRESHOLD, MERGED_MODE_GUIDANCE_THRESHOLD_REPORTS,
    TFlightReportSource, TFlightSnapshot, merged_mode_guidance, unix_epoch_ms_now,
};

pub(super) async fn process_available_reports(
    source: &mut dyn TFlightReportSource,
    health: &HealthStream,
    snapshots: &Arc<RwLock<HashMap<String, TFlightSnapshot>>>,
    states: &mut HashMap<String, DeviceRuntimeState>,
) {
    let paths: Vec<String> = states.keys().cloned().collect();
    for path in paths {
        let read_result = source.read_report(&path);
        let mut messages = HealthMessages::default();
        let mut snapshot_update = None;

        if let Some(state) = states.get_mut(&path) {
            snapshot_update = process_read_result(state, read_result, &mut messages);
        }

        if let Some((snapshot_key, snapshot)) = snapshot_update {
            snapshots.write().await.insert(snapshot_key, snapshot);
        }

        messages.emit(health).await;
    }
}

fn process_read_result(
    state: &mut DeviceRuntimeState,
    read_result: Result<Option<Vec<u8>>, String>,
    messages: &mut HealthMessages,
) -> Option<(String, TFlightSnapshot)> {
    match read_result {
        Ok(Some(report)) => process_report(state, &report, messages),
        Ok(None) => None,
        Err(error) => {
            record_failure(state, "read", error, messages);
            None
        }
    }
}

fn process_report(
    state: &mut DeviceRuntimeState,
    report: &[u8],
    messages: &mut HealthMessages,
) -> Option<(String, TFlightSnapshot)> {
    match state.handler.try_parse_report(report) {
        Ok(parsed) => Some(handle_parsed_report(state, parsed, messages)),
        Err(error) => {
            record_failure(state, "report parse", error.to_string(), messages);
            None
        }
    }
}

fn handle_parsed_report(
    state: &mut DeviceRuntimeState,
    parsed: TFlightInputState,
    messages: &mut HealthMessages,
) -> (String, TFlightSnapshot) {
    state.monitor.record_success();

    let current_mode = state.handler.current_axis_mode();
    track_axis_mode(state, current_mode, messages);

    let yaw = state.handler.resolve_yaw(&parsed);
    let ghost_rate = state.handler.ghost_rate();
    let ghost_stats = state.handler.ghost_stats();
    track_ghost_rate(state, ghost_rate, messages);

    let health_status = state.monitor.status(true, ghost_rate, ghost_stats.clone());
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

    (state.snapshot_key.clone(), snapshot)
}

fn track_axis_mode(
    state: &mut DeviceRuntimeState,
    current_mode: AxisMode,
    messages: &mut HealthMessages,
) {
    if current_mode != state.last_mode {
        messages.info(format!(
            "{} axis mode changed: {} -> {}",
            state.snapshot_key,
            state.last_mode.as_str(),
            current_mode.as_str()
        ));
        state.last_mode = current_mode;
    }

    if current_mode == AxisMode::Merged {
        state.merged_reports_streak = state.merged_reports_streak.saturating_add(1);
        if !state.merged_mode_guidance_emitted
            && state.merged_reports_streak >= MERGED_MODE_GUIDANCE_THRESHOLD_REPORTS
        {
            messages.warning(format!(
                "{} is still in merged axis mode after {} reports. {}",
                state.snapshot_key,
                state.merged_reports_streak,
                merged_mode_guidance(state.model)
            ));
            state.merged_mode_guidance_emitted = true;
        }
    } else {
        state.merged_reports_streak = 0;
        if state.merged_mode_guidance_emitted {
            messages.info(format!(
                "{} full-axis mode detected; merged-mode guidance cleared",
                state.snapshot_key
            ));
            state.merged_mode_guidance_emitted = false;
        }
    }
}

fn track_ghost_rate(
    state: &mut DeviceRuntimeState,
    ghost_rate: f64,
    messages: &mut HealthMessages,
) {
    if ghost_rate > GHOST_WARNING_THRESHOLD && !state.ghost_warning_active {
        messages.warning(format!(
            "{} ghost input rate high: {:.2}%",
            state.snapshot_key,
            ghost_rate * 100.0
        ));
        state.ghost_warning_active = true;
    } else if ghost_rate <= (GHOST_WARNING_THRESHOLD * 0.5) {
        state.ghost_warning_active = false;
    }
}

fn record_failure(
    state: &mut DeviceRuntimeState,
    operation: &str,
    error: String,
    messages: &mut HealthMessages,
) {
    let threshold_reached = state.monitor.record_failure();
    messages.error(format!(
        "{} {} failure: {}",
        state.snapshot_key, operation, error
    ));
    if threshold_reached {
        messages.error(format!(
            "{} {} failures exceeded threshold",
            state.snapshot_key, operation
        ));
    }
}
