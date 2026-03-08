// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2025 Flight Hub Team

//! BDD test runner for Flight Hub specifications
//!
//! This test harness executes Gherkin scenarios defined in specs/features/
//! and validates them against the implementation.

use cucumber::World;

mod steps;

#[derive(Debug, Default, World)]
pub struct FlightWorld {
    // Axis processing state
    pub axis_pipeline: Option<AxisPipelineState>,
    pub scheduler_state: Option<SchedulerState>,
    pub latency_measurements: Vec<f64>,
    pub jitter_measurements: Vec<f64>,

    // Documentation validation state
    pub doc_path: Option<String>,
    pub doc_content: Option<String>,
    pub validation_errors: Vec<String>,
    pub doc_ids: Vec<String>,
    pub front_matter: Option<FrontMatter>,
    pub bdd_traceability: Option<flight_bdd_metrics::BddTraceabilityMetrics>,

    // HOTAS 4 BDD state (REQ-15)
    pub hotas4_handler: Option<flight_hotas_thrustmaster::TFlightInputHandler>,
    pub hotas4_report: Option<Vec<u8>>,
    pub hotas4_parsed_state: Option<flight_hotas_thrustmaster::TFlightInputState>,
    pub hotas4_yaw_resolution: Option<flight_hotas_thrustmaster::TFlightYawResolution>,

    // macOS HID BDD state (REQ-50)
    pub macos_hid_manager: Option<flight_macos_hid::HidManager>,
    pub macos_hid_result: Option<Result<flight_macos_hid::HidManager, flight_macos_hid::HidError>>,
    pub macos_open_error: Option<Option<flight_macos_hid::HidError>>,
    pub macos_device_error: Option<Option<flight_macos_hid::HidError>>,
    pub macos_clock: Option<flight_macos_hid::MacosClock>,
    pub macos_clock_samples: Vec<u64>,
    pub macos_error_string: Option<String>,

    // Open Hardware BDD state (REQ-51)
    pub open_hw_input_buf: Option<Vec<u8>>,
    pub open_hw_input_report: Option<flight_open_hardware::InputReport>,
    pub open_hw_input_parsed: Option<flight_open_hardware::InputReport>,
    pub open_hw_input_roundtrip: Option<flight_open_hardware::InputReport>,
    pub open_hw_ffb_report: Option<flight_open_hardware::FfbOutputReport>,
    pub open_hw_ffb_parsed: Option<flight_open_hardware::FfbOutputReport>,
    pub open_hw_ffb_roundtrip: Option<flight_open_hardware::FfbOutputReport>,
    pub open_hw_ffb_stop: Option<flight_open_hardware::FfbOutputReport>,
    pub open_hw_led_report: Option<flight_open_hardware::LedReport>,
    pub open_hw_led_roundtrip: Option<flight_open_hardware::LedReport>,
    pub open_hw_led_all_off: Option<flight_open_hardware::LedReport>,
    pub open_hw_firmware_report: Option<flight_open_hardware::FirmwareVersionReport>,
    pub open_hw_firmware_roundtrip: Option<flight_open_hardware::FirmwareVersionReport>,
    pub open_hw_firmware_parsed: Option<flight_open_hardware::FirmwareVersionReport>,
    pub open_hw_norms: Option<(f32, f32, f32)>,
    pub open_hw_checked_vendor_id: bool,

    // User journey BDD state (REQ-1051 through REQ-1056)
    pub first_time_setup: Option<steps::user_journeys::FirstTimeSetupState>,
    pub profile_lifecycle: Option<steps::user_journeys::ProfileLifecycleState>,
    pub device_hotplug: Option<steps::user_journeys::DeviceHotplugState>,
    pub sim_adapter: Option<steps::user_journeys::SimAdapterState>,
    pub safe_mode: Option<steps::user_journeys::SafeModeState>,
    pub update_journey: Option<steps::user_journeys::UpdateJourneyState>,
}

#[derive(Debug)]
pub struct AxisPipelineState {
    pub num_axes: usize,
    pub telemetry_rate_hz: u32,
    pub processing_duration_secs: u64,
}

#[derive(Debug)]
pub struct SchedulerState {
    pub rate_hz: u32,
    pub measurement_duration_secs: u64,
    pub warmup_secs: u64,
}

#[derive(Debug, Clone)]
pub struct FrontMatter {
    pub doc_id: String,
    pub kind: String,
    pub area: String,
    pub status: String,
    pub links: Links,
}

#[derive(Debug, Clone, Default)]
pub struct Links {
    pub requirements: Vec<String>,
    pub tasks: Vec<String>,
    pub adrs: Vec<String>,
}

#[tokio::main]
async fn main() {
    // Determine the features path - when running from workspace root
    let features_path = if std::path::Path::new("specs/features").exists() {
        "specs/features/"
    } else {
        // Fallback for when running from specs directory
        "features/"
    };

    FlightWorld::cucumber().run_and_exit(features_path).await;
}
