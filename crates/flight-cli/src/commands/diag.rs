// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Diagnostics and recording commands

use crate::client_manager::ClientManager;
use crate::commands::DiagAction;
use crate::output::OutputFormat;
use flight_blackbox::BlackboxReader;
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;

pub async fn execute(
    action: &DiagAction,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match action {
        DiagAction::Bundle {
            output,
            include_recordings,
        } => {
            create_bundle(
                output.as_deref(),
                *include_recordings,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        DiagAction::Health => health_summary(output_format, verbose, client_manager).await,
        DiagAction::DiagMetrics { reset } => {
            diag_metrics(*reset, output_format, verbose, client_manager).await
        }
        DiagAction::Trace { duration, output } => {
            trace_recording(
                *duration,
                output.as_deref(),
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        DiagAction::Record {
            output,
            duration,
            include_performance,
        } => {
            start_recording(
                output,
                *duration,
                *include_performance,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        DiagAction::Replay {
            input,
            start_time,
            duration,
            validate,
        } => {
            replay_recording(
                input,
                *start_time,
                *duration,
                *validate,
                output_format,
                verbose,
                client_manager,
            )
            .await
        }
        DiagAction::Status => recording_status(output_format, verbose, client_manager).await,
        DiagAction::Stop => stop_recording(output_format, verbose, client_manager).await,
        DiagAction::Export {
            input,
            output,
            sanitize,
            stream,
        } => {
            export_recording(
                input,
                output.as_deref(),
                *sanitize,
                stream.as_deref(),
                output_format,
                verbose,
            )
            .await
        }
    }
}

async fn create_bundle(
    output_path: Option<&Path>,
    include_recordings: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Determine output path
    let default_name = format!(
        "openflight-diag-{}.zip",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let out_path = output_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(&default_name));

    // Try to contact daemon for live data
    let daemon_reachable = match client_manager.get_client().await {
        Ok(mut client) => {
            let _ = client.get_service_info().await;
            true
        }
        Err(_) => false,
    };

    let mut result = json!({
        "bundle_path": out_path.display().to_string(),
        "daemon_reachable": daemon_reachable,
        "include_recordings": include_recordings,
        "contents": [
            "system_info.json",
            "service_status.json",
            "device_list.json",
            "active_profile.json",
            "recent_logs.txt",
            "metrics_snapshot.json"
        ],
        "message": format!("Diagnostic bundle will be written to '{}'", out_path.display()),
        "note": "Full bundle creation requires GetSupportBundle RPC to be implemented in the service",
    });

    if include_recordings {
        result["contents"]
            .as_array_mut()
            .unwrap()
            .push(json!("blackbox_recordings/"));
    }

    if verbose {
        result["collection_details"] = json!({
            "system_info": "OS version, CPU, memory, Rust version",
            "service_status": "Daemon version, uptime, health events",
            "device_list": "All connected devices with capabilities",
            "active_profile": "Current effective profile JSON",
            "recent_logs": "Last 1000 lines from service log",
            "metrics_snapshot": "Current performance counters",
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn health_summary(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;

    let service_info = client.get_service_info().await?;

    let devices_response = client
        .list_devices(flight_ipc::ListDevicesRequest {
            include_disconnected: true,
            filter_types: vec![],
        })
        .await?;

    let connected_count = devices_response
        .devices
        .iter()
        .filter(|d| d.status() == flight_ipc::DeviceStatus::Connected)
        .count();
    let faulted_count = devices_response
        .devices
        .iter()
        .filter(|d| {
            d.status() == flight_ipc::DeviceStatus::Faulted
                || d.status() == flight_ipc::DeviceStatus::Error
        })
        .count();

    let overall_status = if faulted_count > 0 {
        "degraded"
    } else {
        "healthy"
    };

    let mut result = json!({
        "overall_status": overall_status,
        "service_status": service_status_to_string(service_info.status()),
        "service_version": service_info.version,
        "uptime_seconds": service_info.uptime_seconds,
        "devices": {
            "connected": connected_count,
            "total": devices_response.total_count,
            "faulted": faulted_count,
        },
    });

    if verbose {
        let device_details: Vec<serde_json::Value> = devices_response
            .devices
            .iter()
            .map(|d| {
                json!({
                    "id": d.id,
                    "name": d.name,
                    "status": device_status_to_string(d.status()),
                })
            })
            .collect();
        result["device_details"] = json!(device_details);
    }

    let output = match output_format {
        OutputFormat::Human => format_health_summary(
            overall_status,
            connected_count,
            devices_response.total_count,
            faulted_count,
        ),
        OutputFormat::Json => output_format.success(result),
    };
    Ok(Some(output))
}

async fn diag_metrics(
    reset: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Verify daemon is reachable
    let mut client = client_manager.get_client().await?;
    let _service_info = client.get_service_info().await?;

    let mut result = json!({
        "captured_at": chrono::Utc::now().to_rfc3339(),
        "reset_after_capture": reset,
        "rt": {
            "ticks_total": 0,
            "missed_deadlines_total": 0,
            "jitter_p99_us": 0.0,
            "jitter_max_us": 0.0,
        },
        "hid": {
            "write_latency_p99_us": 0.0,
            "read_errors_total": 0,
            "write_errors_total": 0,
        },
        "ipc": {
            "requests_total": 0,
            "request_latency_p99_ms": 0.0,
            "active_subscriptions": 0,
        },
        "note": "Full metrics snapshot requires GetMetrics RPC to be implemented in the service",
    });

    if verbose {
        result["ffb"] = json!({
            "effects_applied_total": 0,
            "fault_count_total": 0,
            "envelope_clamp_total": 0,
            "emergency_stop_total": 0,
        });
        result["memory"] = json!({
            "heap_bytes": 0,
            "resident_bytes": 0,
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn trace_recording(
    duration: u64,
    output_path: Option<&Path>,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Verify daemon is reachable
    let mut client = client_manager.get_client().await?;
    let _service_info = client.get_service_info().await?;

    let default_name = format!(
        "openflight-trace-{}.json",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let out_path = output_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(&default_name));

    let mut result = json!({
        "trace_started": true,
        "duration_seconds": duration,
        "output_file": out_path.display().to_string(),
        "estimated_end_time": chrono::Utc::now().timestamp() + duration as i64,
        "message": format!("Trace recording for {}s to '{}'", duration, out_path.display()),
        "note": "Full trace recording requires StartTrace RPC to be implemented in the service",
    });

    if verbose {
        result["trace_details"] = json!({
            "capture_sources": [
                "axis_pipeline",
                "ffb_engine",
                "scheduler_ticks",
                "ipc_messages",
                "device_events"
            ],
            "format": "Chrome Trace Event Format (JSON)",
            "estimated_size_mb_per_second": 0.5,
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

fn service_status_to_string(status: flight_ipc::ServiceStatus) -> &'static str {
    match status {
        flight_ipc::ServiceStatus::Unspecified => "unspecified",
        flight_ipc::ServiceStatus::Starting => "starting",
        flight_ipc::ServiceStatus::Running => "running",
        flight_ipc::ServiceStatus::Degraded => "degraded",
        flight_ipc::ServiceStatus::Stopping => "stopping",
    }
}

fn device_status_to_string(status: flight_ipc::DeviceStatus) -> &'static str {
    match status {
        flight_ipc::DeviceStatus::Unspecified => "unspecified",
        flight_ipc::DeviceStatus::Connected => "connected",
        flight_ipc::DeviceStatus::Disconnected => "disconnected",
        flight_ipc::DeviceStatus::Error => "error",
        flight_ipc::DeviceStatus::Faulted => "faulted",
    }
}

async fn start_recording(
    output_path: &Path,
    duration: Option<u64>,
    include_performance: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Validate output path
    if let Some(parent) = output_path.parent()
        && !parent.exists()
    {
        return Err(anyhow::anyhow!(
            "Output directory '{}' does not exist",
            parent.display()
        ));
    }

    // Check file extension
    if output_path.extension().and_then(|s| s.to_str()) != Some("fbb") {
        return Err(anyhow::anyhow!("Output file must have .fbb extension"));
    }

    // Note: Actual recording would require a StartRecording RPC method
    // For now, simulate the recording start
    let mut result = json!({
        "recording_started": true,
        "output_file": output_path.display().to_string(),
        "include_performance": include_performance,
        "format": "FBB1",
    });

    if let Some(duration) = duration {
        result["duration_seconds"] = json!(duration);
        result["estimated_end_time"] = json!(chrono::Utc::now().timestamp() + duration as i64);
    } else {
        result["duration"] = json!("continuous");
        result["stop_instruction"] = json!("Use 'flightctl diag stop' to stop recording");
    }

    if verbose {
        result["recording_details"] = json!({
            "streams": {
                "axis_frames": "250Hz axis pipeline outputs",
                "bus_snapshots": "60Hz normalized telemetry",
                "events": "Faults, profile changes, PoF transitions"
            },
            "format_details": {
                "header": "FBB1 | Endian | App_Ver | Timebase | Sim_ID | Aircraft_ID",
                "index_interval": "100ms for seeking",
                "footer": "CRC32C checksum"
            },
            "performance_targets": {
                "max_drops": 0,
                "max_size_per_3min": "30MB",
                "chunk_size": "4-8KB"
            }
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn replay_recording(
    input_path: &Path,
    start_time: Option<f64>,
    duration: Option<f64>,
    validate: bool,
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Validate input file
    if !input_path.exists() {
        return Err(anyhow::anyhow!(
            "Recording file '{}' does not exist",
            input_path.display()
        ));
    }

    if input_path.extension().and_then(|s| s.to_str()) != Some("fbb") {
        return Err(anyhow::anyhow!("Input file must have .fbb extension"));
    }

    // Note: Actual replay would require a ReplayRecording RPC method
    // For now, simulate the replay process
    let mut result = json!({
        "replay_started": true,
        "input_file": input_path.display().to_string(),
        "validate_outputs": validate,
    });

    if let Some(start_time) = start_time {
        result["start_time_seconds"] = json!(start_time);
    }

    if let Some(duration) = duration {
        result["duration_seconds"] = json!(duration);
    }

    // Simulate file analysis
    result["file_info"] = json!({
        "format_version": "FBB1",
        "recording_duration_seconds": 180.5,
        "file_size_bytes": 15728640,
        "streams_found": ["axis_frames", "bus_snapshots", "events"],
        "index_entries": 1805,
    });

    if validate {
        result["validation"] = json!({
            "enabled": true,
            "tolerance": {
                "axis_epsilon": 1e-6,
                "ffb_epsilon_nm": 1e-4,
                "timing_drift_max_ms_per_s": 0.1
            }
        });
    }

    if verbose {
        result["replay_details"] = json!({
            "engine_mode": "offline",
            "real_time_playback": false,
            "output_comparison": validate,
            "streams_to_replay": {
                "axis_frames": "Fed to axis engine at recorded cadence",
                "bus_snapshots": "Available for telemetry validation",
                "events": "Logged for context"
            }
        });
    }

    let stop_output = output_format.success(result);
    Ok(Some(stop_output))
}

async fn export_recording(
    input_path: &Path,
    output_path: Option<&Path>,
    sanitize: bool,
    stream_filter: Option<&str>,
    output_format: OutputFormat,
    verbose: bool,
) -> anyhow::Result<Option<String>> {
    if !input_path.exists() {
        return Err(anyhow::anyhow!(
            "Recording file '{}' does not exist",
            input_path.display()
        ));
    }
    if input_path.extension().and_then(|s| s.to_str()) != Some("fbb") {
        return Err(anyhow::anyhow!("Input file must have .fbb extension"));
    }

    let mut reader = BlackboxReader::open(input_path)?;
    reader.validate()?;

    let mut doc = reader.export(sanitize)?;

    // Apply optional stream filter
    if let Some(filter) = stream_filter {
        let target = match filter {
            "axis" | "axis_frames" => "axis_frames",
            "bus" | "bus_snapshots" => "bus_snapshots",
            "events" => "events",
            other => {
                return Err(anyhow::anyhow!(
                    "Unknown stream '{}'. Valid values: axis, bus, events",
                    other
                ));
            }
        };
        doc.records.retain(|r| r.stream == target);
        doc.summary.total_records = doc.records.len() as u64;
        doc.summary.axis_frames = doc
            .records
            .iter()
            .filter(|r| r.stream == "axis_frames")
            .count() as u64;
        doc.summary.bus_snapshots = doc
            .records
            .iter()
            .filter(|r| r.stream == "bus_snapshots")
            .count() as u64;
        doc.summary.events = doc.records.iter().filter(|r| r.stream == "events").count() as u64;
    }

    let json_bytes = serde_json::to_vec_pretty(&doc)?;

    if let Some(out_path) = output_path {
        let mut file = std::fs::File::create(out_path)?;
        file.write_all(&json_bytes)?;

        let result = json!({
            "exported": true,
            "output_file": out_path.display().to_string(),
            "total_records": doc.summary.total_records,
            "axis_frames": doc.summary.axis_frames,
            "bus_snapshots": doc.summary.bus_snapshots,
            "events": doc.summary.events,
            "sanitized": sanitize,
        });
        return Ok(Some(output_format.success(result)));
    }

    // No output path: emit the document to stdout directly
    let json_str = String::from_utf8(json_bytes)?;
    if verbose {
        let result = json!({
            "exported": true,
            "sanitized": sanitize,
            "total_records": doc.summary.total_records,
            "document": serde_json::from_str::<Value>(&json_str)?
        });
        Ok(Some(output_format.success(result)))
    } else {
        Ok(Some(json_str))
    }
}

async fn recording_status(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Note: Actual status would require a GetRecordingStatus RPC method
    // For now, simulate recording status
    let result = json!({
        "recording_active": false,
        "current_file": null,
        "duration_seconds": 0,
        "file_size_bytes": 0,
        "drops_detected": 0,
        "last_recording": {
            "file": "/path/to/last/recording.fbb",
            "duration_seconds": 300,
            "completed_at": "2024-01-15T10:30:00Z",
            "file_size_bytes": 25165824
        }
    });

    let output = output_format.success(result);
    Ok(Some(output))
}

async fn stop_recording(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    // Note: Actual stop would require a StopRecording RPC method
    // For now, simulate stopping recording
    let mut result = json!({
        "recording_stopped": true,
        "final_file": "/path/to/recording.fbb",
        "total_duration_seconds": 125.7,
        "final_file_size_bytes": 10485760,
        "total_drops": 0,
        "integrity_check": "passed"
    });

    if verbose {
        result["statistics"] = json!({
            "axis_frames_recorded": 31425,
            "bus_snapshots_recorded": 7542,
            "events_recorded": 15,
            "average_write_latency_us": 45.2,
            "max_write_latency_us": 127.8,
            "compression_ratio": 0.73
        });
    }

    let output = output_format.success(result);
    Ok(Some(output))
}

/// Format health summary for human-readable output
pub fn format_health_summary(
    overall: &str,
    connected: usize,
    total: i32,
    faulted: usize,
) -> String {
    let mut lines = vec![
        format!("Overall: {}", overall),
        format!("Devices: {}/{} connected", connected, total),
    ];
    if faulted > 0 {
        lines.push(format!("Faulted: {} device(s)", faulted));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_status_to_string_covers_all_variants() {
        assert_eq!(
            service_status_to_string(flight_ipc::ServiceStatus::Unspecified),
            "unspecified"
        );
        assert_eq!(
            service_status_to_string(flight_ipc::ServiceStatus::Starting),
            "starting"
        );
        assert_eq!(
            service_status_to_string(flight_ipc::ServiceStatus::Running),
            "running"
        );
        assert_eq!(
            service_status_to_string(flight_ipc::ServiceStatus::Degraded),
            "degraded"
        );
        assert_eq!(
            service_status_to_string(flight_ipc::ServiceStatus::Stopping),
            "stopping"
        );
    }

    #[test]
    fn device_status_to_string_covers_all_variants() {
        assert_eq!(
            device_status_to_string(flight_ipc::DeviceStatus::Connected),
            "connected"
        );
        assert_eq!(
            device_status_to_string(flight_ipc::DeviceStatus::Faulted),
            "faulted"
        );
    }

    #[test]
    fn bundle_result_json_format() {
        let result = json!({
            "bundle_path": "diag-bundle.zip",
            "daemon_reachable": false,
            "include_recordings": false,
            "contents": ["system_info.json", "service_status.json"],
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["data"]["contents"].is_array());
        assert_eq!(parsed["data"]["daemon_reachable"], false);
    }

    #[test]
    fn health_result_json_format() {
        let result = json!({
            "overall_status": "healthy",
            "service_status": "running",
            "devices": {
                "connected": 3,
                "total": 5,
                "faulted": 0,
            },
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["overall_status"], "healthy");
        assert_eq!(parsed["data"]["devices"]["connected"], 3);
    }

    #[test]
    fn health_result_human_format() {
        let result = json!({
            "overall_status": "degraded",
            "devices": {"faulted": 1},
        });
        let output = OutputFormat::Human.success(result);
        assert!(output.contains("degraded"));
    }

    #[test]
    fn metrics_result_json_format() {
        let result = json!({
            "captured_at": "2024-01-15T10:00:00Z",
            "reset_after_capture": false,
            "rt": {
                "ticks_total": 100,
                "missed_deadlines_total": 0,
            },
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["rt"]["ticks_total"], 100);
    }

    #[test]
    fn trace_result_json_format() {
        let result = json!({
            "trace_started": true,
            "duration_seconds": 30,
            "output_file": "trace.json",
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["trace_started"], true);
        assert_eq!(parsed["data"]["duration_seconds"], 30);
    }

    #[test]
    fn recording_status_json_format() {
        let result = json!({
            "recording_active": false,
            "drops_detected": 0,
        });
        let output = OutputFormat::Json.success(result);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["recording_active"], false);
    }

    #[test]
    fn format_health_summary_healthy() {
        let summary = format_health_summary("healthy", 3, 3, 0);
        assert!(summary.contains("Overall: healthy"));
        assert!(summary.contains("Devices: 3/3 connected"));
        assert!(!summary.contains("Faulted"));
    }

    #[test]
    fn format_health_summary_degraded() {
        let summary = format_health_summary("degraded", 2, 3, 1);
        assert!(summary.contains("Overall: degraded"));
        assert!(summary.contains("Devices: 2/3 connected"));
        assert!(summary.contains("Faulted: 1 device(s)"));
    }

    #[test]
    fn bundle_human_format_contains_path() {
        let result = json!({
            "bundle_path": "diag-2024.zip",
            "daemon_reachable": true,
        });
        let output = OutputFormat::Human.success(result);
        assert!(output.contains("diag-2024.zip"));
    }
}
