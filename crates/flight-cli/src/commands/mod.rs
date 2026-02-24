// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! CLI command implementations

pub mod ac7;
pub mod cloud_profiles;
pub mod dcs;
pub mod devices;
pub mod diag;
pub mod info;
pub mod metrics;
pub mod overlay;
pub mod panels;
pub mod posture;
pub mod profile;
pub mod sim;
pub mod status;
pub mod torque;
pub mod update;
pub mod xplane;

use clap::{Args, Subcommand};
use std::path::PathBuf;

pub use ac7::Ac7Action;
pub use cloud_profiles::CloudProfilesAction;
pub use dcs::DcsAction;
pub use overlay::OverlayAction;
pub use update::UpdateAction;
pub use xplane::XPlaneAction;

#[derive(Subcommand)]
pub enum DeviceAction {
    /// List all connected devices
    List {
        /// Include disconnected devices
        #[arg(long)]
        include_disconnected: bool,

        /// Filter by device type
        #[arg(long, value_delimiter = ',')]
        filter_types: Vec<String>,
    },
    /// Show detailed information about a specific device
    Info {
        /// Device ID to show information for
        device_id: String,
    },
    /// Dump HID descriptor discovery data for a device
    Dump {
        /// Device ID to dump discovery data for
        device_id: String,
    },
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// Apply a profile from file
    Apply {
        /// Path to profile JSON file
        profile_path: PathBuf,

        /// Only validate the profile without applying
        #[arg(long)]
        validate_only: bool,

        /// Force apply even if validation warnings exist
        #[arg(long)]
        force: bool,
    },
    /// Show current effective profile
    Show {
        /// Show raw JSON instead of formatted output
        #[arg(long)]
        raw: bool,
    },
}

#[derive(Subcommand)]
pub enum SimAction {
    /// Configure simulator integration
    Configure {
        /// Simulator type (msfs, xplane, dcs, ac7)
        sim_type: String,

        /// Configuration action
        #[command(subcommand)]
        action: SimConfigAction,
    },
    /// Detect curve conflicts
    DetectConflicts {
        /// Specific axes to check (default: all)
        #[arg(long, value_delimiter = ',')]
        axes: Vec<String>,

        /// Simulator ID
        #[arg(long)]
        sim_id: Option<String>,

        /// Aircraft ID
        #[arg(long)]
        aircraft_id: Option<String>,
    },
    /// Resolve curve conflicts
    ResolveConflict {
        /// Axis name to resolve
        axis_name: String,

        /// Resolution type (disable-sim-curve, disable-profile-curve, gain-compensation)
        resolution_type: String,

        /// Apply immediately without confirmation
        #[arg(long)]
        apply_immediately: bool,

        /// Create backup before applying
        #[arg(long, default_value = "true")]
        create_backup: bool,
    },
    /// One-click conflict resolution
    OneClickResolve {
        /// Axis name to resolve
        axis_name: String,

        /// Create backup before applying
        #[arg(long, default_value = "true")]
        create_backup: bool,

        /// Verify resolution after applying
        #[arg(long, default_value = "true")]
        verify_resolution: bool,
    },
}

#[derive(Subcommand)]
pub enum SimConfigAction {
    /// Verify current configuration
    Verify,
    /// Repair configuration issues
    Repair {
        /// Apply repairs without confirmation
        #[arg(long)]
        auto_apply: bool,
    },
    /// Rollback to previous configuration
    Rollback {
        /// Backup timestamp or path
        backup_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PanelAction {
    /// Verify panel configuration and LED functionality
    Verify {
        /// Specific panel device ID to verify
        #[arg(long)]
        device_id: Option<String>,

        /// Run extended verification tests
        #[arg(long)]
        extended: bool,
    },
    /// Show panel status and configuration
    Status {
        /// Specific panel device ID
        #[arg(long)]
        device_id: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TorqueAction {
    /// Unlock high torque mode
    Unlock {
        /// Device ID for torque unlock
        device_id: String,

        /// Skip physical button confirmation (for testing)
        #[arg(long)]
        skip_physical_confirm: bool,
    },
    /// Show current torque status
    Status {
        /// Specific device ID
        #[arg(long)]
        device_id: Option<String>,
    },
    /// Set capability mode (full, demo, kid)
    SetMode {
        /// Capability mode to set
        mode: String,

        /// Specific axes to apply to (default: all)
        #[arg(long, value_delimiter = ',')]
        axes: Vec<String>,

        /// Enable audit logging for clamped outputs
        #[arg(long)]
        audit: bool,
    },
}

#[derive(Subcommand)]
pub enum DiagAction {
    /// Start recording diagnostics
    Record {
        /// Output file path for recording
        #[arg(long, short)]
        output: PathBuf,

        /// Duration in seconds (default: continuous until stopped)
        #[arg(long, short)]
        duration: Option<u64>,

        /// Include performance metrics
        #[arg(long)]
        include_performance: bool,
    },
    /// Replay a diagnostics recording
    Replay {
        /// Path to .fbb recording file
        input: PathBuf,

        /// Start time offset in seconds
        #[arg(long)]
        start_time: Option<f64>,

        /// Duration to replay in seconds
        #[arg(long)]
        duration: Option<f64>,

        /// Validate outputs against expected values
        #[arg(long)]
        validate: bool,
    },
    /// Show recording status
    Status,
    /// Export a diagnostics recording to JSON
    Export {
        /// Path to input .fbb recording file
        input: PathBuf,

        /// Write JSON output to this file (prints to stdout if omitted)
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Redact aircraft_id and other identifying fields
        #[arg(long)]
        sanitize: bool,

        /// Only include records from this stream: axis, bus, events
        #[arg(long)]
        stream: Option<String>,
    },
    /// Stop current recording
    Stop,
}

/// Metrics subcommands
#[derive(Subcommand)]
pub enum MetricsAction {
    /// Print a typed snapshot of all system-wide metrics
    Snapshot {
        /// Reset metrics after capturing the snapshot
        #[arg(long)]
        reset: bool,
    },
}
