// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use flight_hotas_thrustmaster::{AxisMode, TFlightInputState};

use crate::health::HealthStream;

use super::{
    COMPONENT_NAME, DeviceRuntimeState, GHOST_WARNING_THRESHOLD,
    MERGED_MODE_GUIDANCE_THRESHOLD_REPORTS, TFlightSnapshot, merged_mode_guidance,
    unix_epoch_ms_now,
};

#[derive(Debug, Default)]
pub(super) struct RuntimeHealthMessages {
    info: Vec<String>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl RuntimeHealthMessages {
    pub(super) fn info(&mut self, message: impl Into<String>) {
        self.info.push(message.into());
    }

    pub(super) fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    pub(super) fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    pub(super) async fn emit(self, health: &HealthStream) {
        for message in self.info {
            health.info(COMPONENT_NAME, &message).await;
        }
        for message in self.warnings {
            health.warning(COMPONENT_NAME, &message).await;
        }
        for message in self.errors {
            health.error(COMPONENT_NAME, &message, None).await;
        }
    }
}

pub(super) fn handle_parsed_report(
    state: &mut DeviceRuntimeState,
    parsed: TFlightInputState,
    messages: &mut RuntimeHealthMessages,
) -> TFlightSnapshot {
    state.monitor.record_success();

    let current_mode = state.handler.current_axis_mode();
    record_axis_mode_change(state, current_mode, messages);
    record_merged_mode_guidance(state, current_mode, messages);

    let yaw = state.handler.resolve_yaw(&parsed);
    let ghost_rate = state.handler.ghost_rate();
    let ghost_stats = state.handler.ghost_stats();
    record_ghost_rate(state, ghost_rate, messages);

    let health_status = state.monitor.status(true, ghost_rate, ghost_stats.clone());
    TFlightSnapshot {
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
    }
}

pub(super) fn record_parse_failure(
    state: &mut DeviceRuntimeState,
    error: impl std::fmt::Display,
    messages: &mut RuntimeHealthMessages,
) {
    let threshold_reached = state.monitor.record_failure();
    messages.error(format!(
        "{} report parse failure: {error}",
        state.snapshot_key
    ));
    if threshold_reached {
        messages.error(format!(
            "{} report parse failures exceeded threshold",
            state.snapshot_key
        ));
    }
}

pub(super) fn record_read_failure(
    state: &mut DeviceRuntimeState,
    error: impl std::fmt::Display,
    messages: &mut RuntimeHealthMessages,
) {
    let threshold_reached = state.monitor.record_failure();
    messages.error(format!("{} read failure: {error}", state.snapshot_key));
    if threshold_reached {
        messages.error(format!(
            "{} read failures exceeded threshold",
            state.snapshot_key
        ));
    }
}

fn record_axis_mode_change(
    state: &mut DeviceRuntimeState,
    current_mode: AxisMode,
    messages: &mut RuntimeHealthMessages,
) {
    if current_mode == state.last_mode {
        return;
    }

    messages.info(format!(
        "{} axis mode changed: {} -> {}",
        state.snapshot_key,
        state.last_mode.as_str(),
        current_mode.as_str()
    ));
    state.last_mode = current_mode;
}

fn record_merged_mode_guidance(
    state: &mut DeviceRuntimeState,
    current_mode: AxisMode,
    messages: &mut RuntimeHealthMessages,
) {
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
        return;
    }

    state.merged_reports_streak = 0;
    if state.merged_mode_guidance_emitted {
        messages.info(format!(
            "{} full-axis mode detected; merged-mode guidance cleared",
            state.snapshot_key
        ));
        state.merged_mode_guidance_emitted = false;
    }
}

fn record_ghost_rate(
    state: &mut DeviceRuntimeState,
    ghost_rate: f64,
    messages: &mut RuntimeHealthMessages,
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
