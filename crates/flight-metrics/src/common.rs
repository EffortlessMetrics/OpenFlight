// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Common metric names used across the workspace.

pub const ADAPTER_UPDATES_TOTAL: &str = "adapter_updates_total";
pub const ADAPTER_ERRORS_TOTAL: &str = "adapter_errors_total";
pub const ADAPTER_UPDATE_LATENCY_MS: &str = "adapter_update_latency_ms";
pub const ADAPTER_TIME_SINCE_LAST_PACKET_MS: &str = "adapter_time_since_last_packet_ms";
pub const DEVICE_OPERATIONS_TOTAL: &str = "device_operations_total";
pub const DEVICE_ERRORS_TOTAL: &str = "device_errors_total";
pub const DEVICE_OPERATION_LATENCY_MS: &str = "device_operation_latency_ms";

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
