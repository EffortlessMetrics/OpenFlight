// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Raw report processing and health-message production for T.Flight ingest.

use flight_hotas_thrustmaster::AxisMode;

use super::{
    COMPONENT_NAME, DeviceRuntimeState, GHOST_WARNING_THRESHOLD,
    MERGED_MODE_GUIDANCE_THRESHOLD_REPORTS, TFlightSnapshot, merged_mode_guidance,
    unix_epoch_ms_now,
};
use crate::health::HealthStream;

#[derive(Debug, Default)]
pub(super) struct ReportProcessingOutcome {
    info_messages: Vec<String>,
    warning_messages: Vec<String>,
    error_messages: Vec<String>,
    snapshot_update: Option<(String, TFlightSnapshot)>,
}

impl ReportProcessingOutcome {
    pub(super) fn snapshot_update(&mut self) -> Option<(String, TFlightSnapshot)> {
        self.snapshot_update.take()
    }

    pub(super) async fn emit_health(self, health: &HealthStream) {
        for message in self.info_messages {
            health.info(COMPONENT_NAME, &message).await;
        }
        for message in self.warning_messages {
            health.warning(COMPONENT_NAME, &message).await;
        }
        for message in self.error_messages {
            health.error(COMPONENT_NAME, &message, None).await;
        }
    }
}

pub(super) fn process_read_result(
    state: &mut DeviceRuntimeState,
    read_result: Result<Option<Vec<u8>>, String>,
) -> ReportProcessingOutcome {
    let mut outcome = ReportProcessingOutcome::default();

    match read_result {
        Ok(Some(report)) => process_report(state, &report, &mut outcome),
        Ok(None) => {}
        Err(error) => record_failure(state, "read", error, &mut outcome),
    }

    outcome
}

fn process_report(
    state: &mut DeviceRuntimeState,
    report: &[u8],
    outcome: &mut ReportProcessingOutcome,
) {
    match state.handler.try_parse_report(report) {
        Ok(parsed) => {
            state.monitor.record_success();

            let current_mode = state.handler.current_axis_mode();
            record_axis_mode_transition(state, current_mode, outcome);
            update_merged_mode_guidance(state, current_mode, outcome);

            let yaw = state.handler.resolve_yaw(&parsed);
            let ghost_rate = state.handler.ghost_rate();
            let ghost_stats = state.handler.ghost_stats();
            update_ghost_warning(state, ghost_rate, outcome);

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

            outcome.snapshot_update = Some((state.snapshot_key.clone(), snapshot));
        }
        Err(error) => record_failure(state, "report parse", error.to_string(), outcome),
    }
}

fn record_axis_mode_transition(
    state: &mut DeviceRuntimeState,
    current_mode: AxisMode,
    outcome: &mut ReportProcessingOutcome,
) {
    if current_mode == state.last_mode {
        return;
    }

    outcome.info_messages.push(format!(
        "{} axis mode changed: {} -> {}",
        state.snapshot_key,
        state.last_mode.as_str(),
        current_mode.as_str()
    ));
    state.last_mode = current_mode;
}

fn update_merged_mode_guidance(
    state: &mut DeviceRuntimeState,
    current_mode: AxisMode,
    outcome: &mut ReportProcessingOutcome,
) {
    if current_mode == AxisMode::Merged {
        state.merged_reports_streak = state.merged_reports_streak.saturating_add(1);
        if !state.merged_mode_guidance_emitted
            && state.merged_reports_streak >= MERGED_MODE_GUIDANCE_THRESHOLD_REPORTS
        {
            outcome.warning_messages.push(format!(
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
            outcome.info_messages.push(format!(
                "{} full-axis mode detected; merged-mode guidance cleared",
                state.snapshot_key
            ));
            state.merged_mode_guidance_emitted = false;
        }
    }
}

fn update_ghost_warning(
    state: &mut DeviceRuntimeState,
    ghost_rate: f64,
    outcome: &mut ReportProcessingOutcome,
) {
    if ghost_rate > GHOST_WARNING_THRESHOLD && !state.ghost_warning_active {
        outcome.warning_messages.push(format!(
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
    outcome: &mut ReportProcessingOutcome,
) {
    let threshold_reached = state.monitor.record_failure();
    outcome.error_messages.push(format!(
        "{} {} failure: {}",
        state.snapshot_key, operation, error
    ));
    if threshold_reached {
        outcome.error_messages.push(format!(
            "{} {} failures exceeded threshold",
            state.snapshot_key, operation
        ));
    }
}
