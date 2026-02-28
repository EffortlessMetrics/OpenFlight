// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Common metric names used across the workspace.

// ── Device / adapter layer ────────────────────────────────────────────────

pub const ADAPTER_UPDATES_TOTAL: &str = "adapter_updates_total";
pub const ADAPTER_ERRORS_TOTAL: &str = "adapter_errors_total";
pub const ADAPTER_UPDATE_LATENCY_MS: &str = "adapter_update_latency_ms";
pub const ADAPTER_TIME_SINCE_LAST_PACKET_MS: &str = "adapter_time_since_last_packet_ms";
pub const DEVICE_OPERATIONS_TOTAL: &str = "device_operations_total";
pub const DEVICE_ERRORS_TOTAL: &str = "device_errors_total";
pub const DEVICE_OPERATION_LATENCY_MS: &str = "device_operation_latency_ms";

// ── Simulator integration (`sim.*`) ──────────────────────────────────────

/// Total frames received from any simulator (counter)
pub const SIM_FRAMES_TOTAL: &str = "sim.frames_total";
/// Total frames that failed to parse or were dropped (counter)
pub const SIM_ERRORS_TOTAL: &str = "sim.errors_total";
/// Current simulator connection state: 1.0 = connected, 0.0 = disconnected (gauge)
pub const SIM_CONNECTION_STATE: &str = "sim.connection_state";
/// Effective data rate from the simulator in Hz (gauge)
pub const SIM_DATA_RATE_HZ: &str = "sim.data_rate_hz";
/// Age of the most recently received packet in milliseconds (gauge)
pub const SIM_LAST_PACKET_AGE_MS: &str = "sim.last_packet_age_ms";
/// Round-trip frame latency from submission to processing, in milliseconds (histogram)
pub const SIM_FRAME_LATENCY_MS: &str = "sim.frame_latency_ms";
/// Number of profile switches triggered by aircraft detection (counter)
pub const SIM_PROFILE_SWITCHES_TOTAL: &str = "sim.profile_switches_total";

// ── Force feedback (`ffb.*`) ──────────────────────────────────────────────

/// Total FFB effect updates applied to the hardware (counter)
pub const FFB_EFFECTS_APPLIED_TOTAL: &str = "ffb.effects_applied_total";
/// Total hardware faults detected by the FFB engine (counter)
pub const FFB_FAULT_COUNT_TOTAL: &str = "ffb.fault_count_total";
/// Times the safety envelope clamped an effect to stay within bounds (counter)
pub const FFB_ENVELOPE_CLAMP_TOTAL: &str = "ffb.envelope_clamp_total";
/// Times an emergency-stop was triggered (counter)
pub const FFB_EMERGENCY_STOP_TOTAL: &str = "ffb.emergency_stop_total";
/// Configured maximum output torque in N·m (gauge)
pub const FFB_MAX_TORQUE_NM: &str = "ffb.max_torque_nm";
/// Current output torque in N·m (gauge)
pub const FFB_CURRENT_TORQUE_NM: &str = "ffb.current_torque_nm";
/// Effect write latency from calculation to hardware delivery, in milliseconds (histogram)
pub const FFB_EFFECT_LATENCY_MS: &str = "ffb.effect_latency_ms";

// ── Real-time scheduler (`rt.*`) ──────────────────────────────────────────

/// 250 Hz scheduler ticks processed since start (counter)
pub const RT_TICKS_TOTAL: &str = "rt.ticks_total";
/// Scheduler ticks that missed their deadline (counter)
pub const RT_MISSED_DEADLINES_TOTAL: &str = "rt.missed_deadlines_total";
/// Tick interval jitter in microseconds (histogram)
pub const RT_JITTER_US: &str = "rt.jitter_us";

// ── Axis processing (`axis.*`) ────────────────────────────────────────────

/// Axis processing latency per tick in microseconds (histogram)
pub const AXIS_PROCESSING_LATENCY_US: &str = "axis.processing_latency_us";

// ── Bus events (`bus.*`) ──────────────────────────────────────────────────

/// Bus events dispatched per second (gauge)
pub const BUS_EVENTS_PER_SECOND: &str = "bus.events_per_second";
/// Total bus events dispatched (counter)
pub const BUS_EVENTS_TOTAL: &str = "bus.events_total";

// ── Device inventory (`devices.*`) ────────────────────────────────────────

/// Number of currently connected devices (gauge)
pub const DEVICES_CONNECTED_COUNT: &str = "devices.connected_count";

// ── Watchdog (`watchdog.*`) ───────────────────────────────────────────────

/// Dead-man's switch triggers (counter)
pub const WATCHDOG_DMS_TRIGGERS_TOTAL: &str = "watchdog.dms_triggers_total";
/// Hardware watchdog timeouts (counter)
pub const WATCHDOG_HW_TIMEOUTS_TOTAL: &str = "watchdog.hw_timeouts_total";

/// Metric names for device operations grouped by layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceMetricNames {
    pub operations_total: &'static str,
    pub errors_total: &'static str,
    pub operation_latency_ms: &'static str,
}

/// Shared device metric names (legacy aggregated view).
pub const DEVICE_METRICS_SHARED: DeviceMetricNames = DeviceMetricNames {
    operations_total: DEVICE_OPERATIONS_TOTAL,
    errors_total: DEVICE_ERRORS_TOTAL,
    operation_latency_ms: DEVICE_OPERATION_LATENCY_MS,
};

/// HID layer device metrics.
pub const HID_DEVICE_METRICS: DeviceMetricNames = DeviceMetricNames {
    operations_total: "hid_device_operations_total",
    errors_total: "hid_device_errors_total",
    operation_latency_ms: "hid_device_operation_latency_ms",
};

/// Panel writer device metrics.
pub const PANEL_DEVICE_METRICS: DeviceMetricNames = DeviceMetricNames {
    operations_total: "panel_device_operations_total",
    errors_total: "panel_device_errors_total",
    operation_latency_ms: "panel_device_operation_latency_ms",
};

/// FFB device metrics.
pub const FFB_DEVICE_METRICS: DeviceMetricNames = DeviceMetricNames {
    operations_total: "ffb_device_operations_total",
    errors_total: "ffb_device_errors_total",
    operation_latency_ms: "ffb_device_operation_latency_ms",
};
