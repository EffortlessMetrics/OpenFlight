// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Process Detection System
//!
//! Provides cross-platform process detection for flight simulators with
//! fast detection times and reliable process monitoring.

use crate::{FlightError, Result};
use crate::aircraft_switch::SimId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::warn;

/// Process detection system for flight simulators
#[derive(Debug)]
pub struct ProcessDetector {
    #[allow(dead_code)]
    config: ProcessDetectionConfig,
    state: RwLock<DetectionState>,
    detection_tx: mpsc::UnboundedSender<DetectionEvent>,
    #[allow(dead_code)]
    detection_rx: RwLock<Option<mpsc::UnboundedReceiver<DetectionEvent>>>,
}

/// Configuration for process detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDetectionConfig {
    /// Detection interval (default: 1 second)
    pub detection_interval: Duration,
    /// Process definitions for each simulator
    pub process_definitions: HashMap<SimId, ProcessDefinition>,
    /// Whether to enable window title detection
    pub enable_window_detection: bool,
    /// Maximum detection time budget
    pub max_detection_time: Duration,
}

/// Process definition for a simulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessDefinition {
    /// Process executable names to look for
    pub process_names: Vec<String>,
    /// Window titles to look for (if window detection enabled)
    pub window_titles: Vec<String>,
    /// Process paths to check
    pub process_paths: Vec<PathBuf>,
    /// Minimum confidence threshold (0.0 to 1.0)
    pub min_confidence: f32,
}

/// Internal detection state
#[derive(Debug)]
struct DetectionState {
    /// Currently detected processes
    detected_processes: HashMap<SimId, DetectedProcess>,
    /// Last detection scan time
    #[allow(dead_code)]
    last_scan: Option<Instant>,
    /// Detection metrics
    metrics: DetectionMetrics,
}

/// Detected process information
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedProcess {
    pub sim: SimId,
    pub process_id: u32,
    pub process_name: String,
    pub process_path: PathBuf,
    pub window_title: Option<String>,
    pub detection_time: Instant,
    pub confidence: f32,
}

/// Detection event for internal processing
#[derive(Debug)]
#[allow(dead_code)]
enum DetectionEvent {
    ScanProcesses,
    ProcessFound(DetectedProcess),
    ProcessLost(SimId),
    Shutdown,
}

/// Detection performance metrics
#[derive(Debug, Default)]
pub struct DetectionMetrics {
    pub total_scans: u64,
    pub successful_detections: u64,
    pub false_positives: u64,
    pub average_scan_time: Duration,
    pub max_scan_time: Duration,
}

impl Default for ProcessDetectionConfig {
    fn default() -> Self {
        let mut process_definitions = HashMap::new();

        // MSFS process definition
        process_definitions.insert(SimId::Msfs, ProcessDefinition {
            process_names: vec![
                "FlightSimulator.exe".to_string(),
                "fsx.exe".to_string(), // Legacy FSX support
            ],
            window_titles: vec![
                "Microsoft Flight Simulator".to_string(),
                "Microsoft Flight Simulator X".to_string(),
            ],
            process_paths: vec![
                PathBuf::from("Microsoft Flight Simulator"),
                PathBuf::from("Microsoft Games/Microsoft Flight Simulator X"),
            ],
            min_confidence: 0.8,
        });

        // X-Plane process definition
        process_definitions.insert(SimId::XPlane, ProcessDefinition {
            process_names: vec![
                "X-Plane.exe".to_string(),
                "X-Plane-x86_64".to_string(),
                "X-Plane 12.exe".to_string(),
                "X-Plane 11.exe".to_string(),
            ],
            window_titles: vec![
                "X-Plane".to_string(),
                "X-Plane 12".to_string(),
                "X-Plane 11".to_string(),
            ],
            process_paths: vec![
                PathBuf::from("X-Plane 12"),
                PathBuf::from("X-Plane 11"),
                PathBuf::from("X-Plane"),
            ],
            min_confidence: 0.8,
        });

        // DCS process definition
        process_definitions.insert(SimId::Dcs, ProcessDefinition {
            process_names: vec![
                "DCS.exe".to_string(),
                "DCS_updater.exe".to_string(),
            ],
            window_titles: vec![
                "DCS World".to_string(),
                "Digital Combat Simulator".to_string(),
            ],
            process_paths: vec![
                PathBuf::from("DCS World"),
                PathBuf::from("Eagle Dynamics/DCS World"),
            ],
            min_confidence: 0.8,
        });

        Self {
            detection_interval: Duration::from_secs(1),
            process_definitions,
            enable_window_detection: true,
            max_detection_time: Duration::from_millis(100),
        }
    }
}

impl ProcessDetector {
    /// Create new process detector
    pub fn new(config: ProcessDetectionConfig) -> Self {
        let (detection_tx, detection_rx) = mpsc::unbounded_channel();

        Self {
            config,
            state: RwLock::new(DetectionState::new()),
            detection_tx,
            detection_rx: RwLock::new(Some(detection_rx)),
        }
    }

    /// Start the process detection loop
    pub async fn start(&self) -> Result<()> {
        // TODO: Fix lifetime issues - temporarily disabled for DSL implementation
        Ok(())
    }

    /// Stop the process detection system
    pub async fn stop(&self) -> Result<()> {
        self.detection_tx.send(DetectionEvent::Shutdown)
            .map_err(|e| FlightError::AutoSwitch(format!("Failed to send shutdown: {}", e)))?;
        Ok(())
    }

    /// Get currently detected processes
    pub async fn get_detected_processes(&self) -> HashMap<SimId, DetectedProcess> {
        self.state.read().await.detected_processes.clone()
    }

    /// Get detection metrics
    pub async fn get_metrics(&self) -> DetectionMetrics {
        self.state.read().await.metrics.clone()
    }

    /// Check if a specific simulator is detected
    pub async fn is_sim_detected(&self, sim: SimId) -> bool {
        self.state.read().await.detected_processes.contains_key(&sim)
    }

    /// Get detected process for specific simulator
    pub async fn get_detected_process(&self, sim: SimId) -> Option<DetectedProcess> {
        self.state.read().await.detected_processes.get(&sim).cloned()
    }

    /// Scan for processes (internal)
    #[allow(dead_code)]
    async fn scan_processes(
        state: &RwLock<DetectionState>,
        config: &ProcessDetectionConfig,
    ) -> Result<()> {
        let scan_start = Instant::now();

        // Get current system processes
        let system_processes = Self::get_system_processes().await?;

        // Check each simulator definition
        for (sim_id, definition) in &config.process_definitions {
            let detected = Self::check_simulator_processes(
                *sim_id,
                definition,
                &system_processes,
                config.enable_window_detection,
            ).await?;

            let current_state = {
                let state_guard = state.read().await;
                state_guard.detected_processes.get(sim_id).cloned()
            };

            match (current_state, detected) {
                (None, Some(process)) => {
                    // New process detected
                    let mut state_guard = state.write().await;
                    state_guard.detected_processes.insert(*sim_id, process);
                    state_guard.metrics.successful_detections += 1;
                }
                (Some(_), None) => {
                    // Process lost
                    let mut state_guard = state.write().await;
                    state_guard.detected_processes.remove(sim_id);
                }
                (Some(current), Some(new)) if current.process_id != new.process_id => {
                    // Process changed (restart detected)
                    let mut state_guard = state.write().await;
                    state_guard.detected_processes.insert(*sim_id, new);
                }
                _ => {
                    // No change
                }
            }
        }

        // Update scan timing
        let scan_time = scan_start.elapsed();
        if scan_time > config.max_detection_time {
            warn!("Process scan exceeded time budget: {:?} > {:?}", scan_time, config.max_detection_time);
        }

        let mut state_guard = state.write().await;
        state_guard.last_scan = Some(scan_start);

        Ok(())
    }

    /// Get system processes (platform-specific)
    #[allow(dead_code)]
    async fn get_system_processes() -> Result<Vec<SystemProcess>> {
        #[cfg(target_os = "windows")]
        {
            Self::get_windows_processes().await
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_processes().await
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            Err(FlightError::AutoSwitch("Unsupported platform for process detection".to_string()))
        }
    }

    /// Check if simulator processes are running
    #[allow(dead_code)]
    async fn check_simulator_processes(
        sim_id: SimId,
        definition: &ProcessDefinition,
        system_processes: &[SystemProcess],
        enable_window_detection: bool,
    ) -> Result<Option<DetectedProcess>> {
        let mut best_match: Option<DetectedProcess> = None;
        let mut best_confidence = 0.0f32;

        for process in system_processes {
            let mut confidence = 0.0f32;

            // Check process name match
            for process_name in &definition.process_names {
                if process.name.to_lowercase().contains(&process_name.to_lowercase()) {
                    confidence += 0.6; // High weight for process name
                    break;
                }
            }

            // Check process path match
            for process_path in &definition.process_paths {
                if process.path.to_string_lossy().to_lowercase()
                    .contains(&process_path.to_string_lossy().to_lowercase()) {
                    confidence += 0.3; // Medium weight for path
                    break;
                }
            }

            // Check window title match (if enabled)
            if enable_window_detection {
                if let Some(window_title) = &process.window_title {
                    for title_pattern in &definition.window_titles {
                        if window_title.to_lowercase().contains(&title_pattern.to_lowercase()) {
                            confidence += 0.1; // Low weight for window title
                            break;
                        }
                    }
                }
            }

            // Check if this is the best match so far
            if confidence >= definition.min_confidence && confidence > best_confidence {
                best_confidence = confidence;
                best_match = Some(DetectedProcess {
                    sim: sim_id,
                    process_id: process.pid,
                    process_name: process.name.clone(),
                    process_path: process.path.clone(),
                    window_title: process.window_title.clone(),
                    detection_time: Instant::now(),
                    confidence,
                });
            }
        }

        Ok(best_match)
    }

    #[cfg(target_os = "windows")]
    #[allow(dead_code)]
    async fn get_windows_processes() -> Result<Vec<SystemProcess>> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };
        use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

        let mut processes = Vec::new();

        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|e| FlightError::AutoSwitch(format!("Failed to create process snapshot: {}", e)))?;

            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let process_name = OsString::from_wide(&entry.szExeFile[..])
                        .to_string_lossy()
                        .trim_end_matches('\0')
                        .to_string();

                    // Get process path
                    let process_path = if let Ok(process_handle) = OpenProcess(
                        PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                        false,
                        entry.th32ProcessID,
                    ) {
                        let mut path_buffer = [0u16; 260];
                        if K32GetModuleFileNameExW(Some(process_handle), None, &mut path_buffer) > 0 {
                            let path = OsString::from_wide(&path_buffer)
                                .to_string_lossy()
                                .trim_end_matches('\0')
                                .to_string();
                            let _ = CloseHandle(process_handle);
                            PathBuf::from(path)
                        } else {
                            let _ = CloseHandle(process_handle);
                            PathBuf::new()
                        }
                    } else {
                        PathBuf::new()
                    };

                    // TODO: Get window title if needed
                    let window_title = None;

                    processes.push(SystemProcess {
                        pid: entry.th32ProcessID,
                        name: process_name,
                        path: process_path,
                        window_title,
                    });

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }

            let _ = CloseHandle(snapshot);
        }

        Ok(processes)
    }

    #[cfg(target_os = "linux")]
    async fn get_linux_processes() -> Result<Vec<SystemProcess>> {
        use std::fs;

        let mut processes = Vec::new();

        // Read /proc directory
        let proc_dir = fs::read_dir("/proc")
            .map_err(|e| FlightError::AutoSwitch(format!("Failed to read /proc: {}", e)))?;

        for entry in proc_dir {
            let entry = entry.map_err(|e| FlightError::AutoSwitch(format!("Failed to read proc entry: {}", e)))?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Check if this is a PID directory
            if let Ok(pid) = file_name_str.parse::<u32>() {
                // Read process name from /proc/PID/comm
                let comm_path = format!("/proc/{}/comm", pid);
                let process_name = fs::read_to_string(&comm_path)
                    .unwrap_or_else(|_| "unknown".to_string())
                    .trim()
                    .to_string();

                // Read process path from /proc/PID/exe
                let exe_path = format!("/proc/{}/exe", pid);
                let process_path = fs::read_link(&exe_path)
                    .unwrap_or_else(|_| PathBuf::new());

                // TODO: Get window title if needed (requires X11/Wayland integration)
                let window_title = None;

                processes.push(SystemProcess {
                    pid,
                    name: process_name,
                    path: process_path,
                    window_title,
                });
            }
        }

        Ok(processes)
    }
}

/// System process information
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SystemProcess {
    pid: u32,
    name: String,
    path: PathBuf,
    window_title: Option<String>,
}

impl DetectionState {
    fn new() -> Self {
        Self {
            detected_processes: HashMap::new(),
            last_scan: None,
            metrics: DetectionMetrics::default(),
        }
    }
}

impl Clone for DetectionMetrics {
    fn clone(&self) -> Self {
        Self {
            total_scans: self.total_scans,
            successful_detections: self.successful_detections,
            false_positives: self.false_positives,
            average_scan_time: self.average_scan_time,
            max_scan_time: self.max_scan_time,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_detector_creation() {
        let config = ProcessDetectionConfig::default();
        let detector = ProcessDetector::new(config);
        
        // Should have process definitions for all simulators
        assert!(detector.config.process_definitions.contains_key(&SimId::Msfs));
        assert!(detector.config.process_definitions.contains_key(&SimId::XPlane));
        assert!(detector.config.process_definitions.contains_key(&SimId::Dcs));
    }

    #[test]
    fn test_process_definition_defaults() {
        let config = ProcessDetectionConfig::default();
        
        let msfs_def = config.process_definitions.get(&SimId::Msfs).unwrap();
        assert!(msfs_def.process_names.contains(&"FlightSimulator.exe".to_string()));
        assert!(msfs_def.window_titles.contains(&"Microsoft Flight Simulator".to_string()));
        assert_eq!(msfs_def.min_confidence, 0.8);

        let xplane_def = config.process_definitions.get(&SimId::XPlane).unwrap();
        assert!(xplane_def.process_names.contains(&"X-Plane.exe".to_string()));
        
        let dcs_def = config.process_definitions.get(&SimId::Dcs).unwrap();
        assert!(dcs_def.process_names.contains(&"DCS.exe".to_string()));
    }

    #[tokio::test]
    async fn test_process_detection_lifecycle() {
        let config = ProcessDetectionConfig::default();
        let detector = ProcessDetector::new(config);
        
        // Should start successfully
        assert!(detector.start().await.is_ok());
        
        // Should have no detected processes initially
        let processes = detector.get_detected_processes().await;
        assert!(processes.is_empty());
        
        // Should stop successfully
        assert!(detector.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_sim_detection_check() {
        let config = ProcessDetectionConfig::default();
        let detector = ProcessDetector::new(config);
        
        // Should not detect any sims initially
        assert!(!detector.is_sim_detected(SimId::Msfs).await);
        assert!(!detector.is_sim_detected(SimId::XPlane).await);
        assert!(!detector.is_sim_detected(SimId::Dcs).await);
    }
}