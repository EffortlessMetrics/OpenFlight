// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Process Detection System
//!
//! Provides cross-platform process detection for flight simulators with
//! fast detection times and reliable process monitoring.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn};

/// Error type for process detection
#[derive(Debug, Error)]
pub enum ProcessDetectionError {
    #[error("Platform error: {0}")]
    Platform(String),
    #[error("System error: {0}")]
    System(String),
}

pub type Result<T> = std::result::Result<T, ProcessDetectionError>;

/// Simulator identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SimId {
    Msfs,
    Msfs2024,
    XPlane,
    Dcs,
    AceCombat7,
    WarThunder,
    EliteDangerous,
    Ksp,
    Wingman,
    Unknown,
}

impl std::fmt::Display for SimId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimId::Msfs => write!(f, "MSFS"),
            SimId::Msfs2024 => write!(f, "MSFS 2024"),
            SimId::XPlane => write!(f, "X-Plane"),
            SimId::Dcs => write!(f, "DCS"),
            SimId::AceCombat7 => write!(f, "Ace Combat 7"),
            SimId::WarThunder => write!(f, "War Thunder"),
            SimId::EliteDangerous => write!(f, "Elite: Dangerous"),
            SimId::Ksp => write!(f, "Kerbal Space Program"),
            SimId::Wingman => write!(f, "Project Wingman"),
            SimId::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Process detection system for flight simulators
#[derive(Debug)]
pub struct ProcessDetector {
    config: ProcessDetectionConfig,
    state: RwLock<DetectionState>,
    detection_tx: mpsc::UnboundedSender<DetectionEvent>,
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

        // MSFS process definition (covers MSFS 2020 / FSX by window title)
        process_definitions.insert(
            SimId::Msfs,
            ProcessDefinition {
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
            },
        );

        // MSFS 2024 process definition (same exe, distinguished by window title)
        process_definitions.insert(
            SimId::Msfs2024,
            ProcessDefinition {
                process_names: vec![
                    "FlightSimulator2024.exe".to_string(),
                    "FlightSimulator.exe".to_string(),
                ],
                window_titles: vec!["Microsoft Flight Simulator 2024".to_string()],
                process_paths: vec![PathBuf::from("Microsoft Flight Simulator 2024")],
                min_confidence: 0.8,
            },
        );

        // X-Plane process definition
        process_definitions.insert(
            SimId::XPlane,
            ProcessDefinition {
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
            },
        );

        // DCS process definition
        process_definitions.insert(
            SimId::Dcs,
            ProcessDefinition {
                process_names: vec!["DCS.exe".to_string(), "DCS_updater.exe".to_string()],
                window_titles: vec![
                    "DCS World".to_string(),
                    "Digital Combat Simulator".to_string(),
                ],
                process_paths: vec![
                    PathBuf::from("DCS World"),
                    PathBuf::from("Eagle Dynamics/DCS World"),
                ],
                min_confidence: 0.8,
            },
        );

        // Ace Combat 7 process definition
        process_definitions.insert(
            SimId::AceCombat7,
            ProcessDefinition {
                process_names: vec!["acecombat7.exe".to_string(), "ACE7Game.exe".to_string()],
                window_titles: vec!["ACE COMBAT 7".to_string(), "SKIES UNKNOWN".to_string()],
                process_paths: vec![
                    PathBuf::from("ACE COMBAT 7"),
                    PathBuf::from("steamapps/common/ACE COMBAT 7"),
                ],
                min_confidence: 0.8,
            },
        );

        // War Thunder process definition
        process_definitions.insert(
            SimId::WarThunder,
            ProcessDefinition {
                process_names: vec![
                    "aces.exe".to_string(),
                    "WarThunder.exe".to_string(),
                    "WarThunder".to_string(),
                ],
                window_titles: vec!["War Thunder".to_string()],
                process_paths: vec![
                    PathBuf::from("War Thunder"),
                    PathBuf::from("steamapps/common/War Thunder"),
                ],
                min_confidence: 0.8,
            },
        );

        // Elite: Dangerous process definition
        process_definitions.insert(
            SimId::EliteDangerous,
            ProcessDefinition {
                process_names: vec![
                    "EliteDangerous64.exe".to_string(),
                    "EliteDangerous.exe".to_string(),
                    "EliteDangerous_4_0_0_0".to_string(),
                ],
                window_titles: vec![
                    "Elite - Dangerous".to_string(),
                    "Elite Dangerous".to_string(),
                ],
                process_paths: vec![
                    PathBuf::from("Elite Dangerous"),
                    PathBuf::from("steamapps/common/Elite Dangerous"),
                    PathBuf::from("Frontier Developments/Elite Dangerous"),
                ],
                min_confidence: 0.8,
            },
        );

        // Kerbal Space Program process definition
        process_definitions.insert(
            SimId::Ksp,
            ProcessDefinition {
                process_names: vec![
                    "KSP_x64.exe".to_string(),  // Windows
                    "KSP.x86_64".to_string(),   // Linux
                    "KSP.app".to_string(),      // macOS
                    "KSP2_x64.exe".to_string(), // KSP 2 (future-proofing)
                ],
                window_titles: vec![
                    "Kerbal Space Program".to_string(),
                    "Kerbal Space Program 2".to_string(),
                ],
                process_paths: vec![
                    PathBuf::from("Kerbal Space Program"),
                    PathBuf::from("steamapps/common/Kerbal Space Program"),
                    PathBuf::from("steamapps/common/Kerbal Space Program 2"),
                ],
                min_confidence: 0.8,
            },
        );

        // Project Wingman process definition
        process_definitions.insert(
            SimId::Wingman,
            ProcessDefinition {
                process_names: vec!["ProjectWingman.exe".to_string()],
                window_titles: vec!["Project Wingman".to_string()],
                process_paths: vec![
                    PathBuf::from("Project Wingman"),
                    PathBuf::from("steamapps/common/Project Wingman"),
                ],
                min_confidence: 0.7,
            },
        );

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

    /// Start the process detection loop.
    ///
    /// Takes `Arc<Self>` so the spawned task can share ownership of the detector's
    /// state and config without lifetime restrictions.
    pub async fn start(self: Arc<Self>) -> Result<()> {
        // Take the receiver out of the Option — ensures start() is called at most once.
        let rx = {
            let mut rx_guard = self.detection_rx.write().await;
            rx_guard.take().ok_or_else(|| {
                ProcessDetectionError::System("ProcessDetector already started".to_string())
            })?
        };

        let detector = Arc::clone(&self);
        let interval_duration = self.config.detection_interval;

        tokio::spawn(async move {
            let mut rx = rx;
            let mut ticker = tokio::time::interval(interval_duration);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        if let Err(e) = Self::scan_processes(&detector.state, &detector.config).await {
                            warn!("Process scan error: {e}");
                        }
                    }
                    Some(event) = rx.recv() => {
                        if let DetectionEvent::Shutdown = event {
                            info!("Process detector shutting down");
                            break;
                        }
                    }
                    else => break,
                }
            }
        });

        Ok(())
    }

    /// Perform one process scan cycle immediately and update detector state.
    pub async fn scan_once(&self) -> Result<()> {
        Self::scan_processes(&self.state, &self.config).await
    }

    /// Stop the process detection system
    pub async fn stop(&self) -> Result<()> {
        self.detection_tx
            .send(DetectionEvent::Shutdown)
            .map_err(|e| {
                ProcessDetectionError::System(format!("Failed to send shutdown: {}", e))
            })?;
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
        self.state
            .read()
            .await
            .detected_processes
            .contains_key(&sim)
    }

    /// Get detected process for specific simulator
    pub async fn get_detected_process(&self, sim: SimId) -> Option<DetectedProcess> {
        self.state
            .read()
            .await
            .detected_processes
            .get(&sim)
            .cloned()
    }

    /// Scan for processes (internal)
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
            )
            .await?;

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
            warn!(
                "Process scan exceeded time budget: {:?} > {:?}",
                scan_time, config.max_detection_time
            );
        }

        let mut state_guard = state.write().await;
        state_guard.last_scan = Some(scan_start);
        state_guard.metrics.total_scans += 1;
        state_guard.metrics.max_scan_time = state_guard.metrics.max_scan_time.max(scan_time);

        let scan_count = state_guard.metrics.total_scans as f64;
        let previous_count = (state_guard.metrics.total_scans - 1) as f64;
        let previous_avg = state_guard.metrics.average_scan_time.as_secs_f64();
        let new_avg =
            ((previous_avg * previous_count) + scan_time.as_secs_f64()) / scan_count.max(1.0);
        state_guard.metrics.average_scan_time = Duration::from_secs_f64(new_avg);

        Ok(())
    }

    /// Get system processes (platform-specific)
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
            Err(ProcessDetectionError::Platform(
                "Unsupported platform for process detection".to_string(),
            ))
        }
    }

    /// Check if simulator processes are running
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
                if process
                    .name
                    .to_lowercase()
                    .contains(&process_name.to_lowercase())
                {
                    confidence += 0.6; // High weight for process name
                    break;
                }
            }

            // Check process path match
            for process_path in &definition.process_paths {
                if process
                    .path
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&process_path.to_string_lossy().to_lowercase())
                {
                    confidence += 0.3; // Medium weight for path
                    break;
                }
            }

            // Check window title match (if enabled)
            if enable_window_detection && let Some(window_title) = &process.window_title {
                for title_pattern in &definition.window_titles {
                    if window_title
                        .to_lowercase()
                        .contains(&title_pattern.to_lowercase())
                    {
                        confidence += 0.1; // Low weight for window title
                        break;
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
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        };
        use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
        use windows::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };

        let mut processes = Vec::new();

        // First, collect all window titles by process ID
        let window_titles = Self::get_windows_window_titles();

        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| {
                ProcessDetectionError::System(format!("Failed to create process snapshot: {}", e))
            })?;

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
                        if K32GetModuleFileNameExW(Some(process_handle), None, &mut path_buffer) > 0
                        {
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

                    // Get window title for this process (use the first non-empty title found)
                    let window_title = window_titles.get(&entry.th32ProcessID).cloned();

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

    /// Get window titles for all processes on Windows.
    /// Returns a HashMap mapping process ID to the window title of its main window.
    #[cfg(target_os = "windows")]
    fn get_windows_window_titles() -> HashMap<u32, String> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        use std::sync::Mutex;
        use windows::Win32::Foundation::{HWND, LPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
            IsWindowVisible,
        };
        use windows::core::BOOL;

        // Thread-local storage for collecting window titles during enumeration
        static WINDOW_TITLES: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);

        // Initialize the collection
        {
            let mut titles = WINDOW_TITLES.lock().unwrap();
            *titles = Some(HashMap::new());
        }

        // Callback function for EnumWindows
        unsafe extern "system" fn enum_windows_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
            // Only process visible windows with non-empty titles
            if unsafe { IsWindowVisible(hwnd).as_bool() } {
                let title_len = unsafe { GetWindowTextLengthW(hwnd) };
                if title_len > 0 {
                    // Get the process ID for this window
                    let mut process_id: u32 = 0;
                    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };

                    if process_id != 0 {
                        // Get the window title
                        let mut title_buffer = vec![0u16; (title_len + 1) as usize];
                        let actual_len = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };

                        if actual_len > 0 {
                            let title = OsString::from_wide(&title_buffer[..actual_len as usize])
                                .to_string_lossy()
                                .to_string();

                            // Skip empty titles and system windows with generic names
                            if !title.is_empty()
                                && !title.starts_with("MSCTFIME")
                                && !title.starts_with("Default IME")
                                && let Ok(mut titles) = WINDOW_TITLES.lock()
                                && let Some(map) = titles.as_mut()
                            {
                                // Only store the first (typically main) window title
                                map.entry(process_id).or_insert(title);
                            }
                        }
                    }
                }
            }
            BOOL(1) // Continue enumeration (TRUE)
        }

        // Enumerate all top-level windows
        unsafe {
            let _ = EnumWindows(Some(enum_windows_callback), LPARAM(0));
        }

        // Extract and return the collected titles
        let mut titles = WINDOW_TITLES.lock().unwrap();
        titles.take().unwrap_or_default()
    }

    #[cfg(target_os = "linux")]
    async fn get_linux_processes() -> Result<Vec<SystemProcess>> {
        use std::fs;

        let mut processes = Vec::new();

        // Get window titles from X11 (if available)
        let window_titles = Self::get_linux_window_titles();

        // Read /proc directory
        let proc_dir = fs::read_dir("/proc")
            .map_err(|e| ProcessDetectionError::System(format!("Failed to read /proc: {}", e)))?;

        for entry in proc_dir {
            let entry = entry.map_err(|e| {
                ProcessDetectionError::System(format!("Failed to read proc entry: {}", e))
            })?;
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
                let process_path = fs::read_link(&exe_path).unwrap_or_else(|_| PathBuf::new());

                // Get window title for this process from X11 data
                let window_title = window_titles.get(&pid).cloned();

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

    /// Get window titles for all processes on Linux via X11.
    /// Returns a HashMap mapping process ID to window title.
    ///
    /// Note: This only works for X11 sessions. On Wayland, window title
    /// detection is not possible without compositor-specific protocols,
    /// as Wayland intentionally restricts cross-client window inspection
    /// for security reasons.
    #[cfg(target_os = "linux")]
    fn get_linux_window_titles() -> HashMap<u32, String> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as XprotoConnectionExt};

        let mut window_titles = HashMap::new();

        // Try to connect to X11 display
        let Ok((conn, screen_num)) = x11rb::connect(None) else {
            // X11 not available (possibly running on Wayland without XWayland)
            tracing::debug!("X11 connection failed, window titles unavailable");
            return window_titles;
        };

        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        // Get atoms we need
        let Ok(net_wm_pid_cookie) = conn.intern_atom(false, b"_NET_WM_PID") else {
            return window_titles;
        };
        let Ok(net_wm_name_cookie) = conn.intern_atom(false, b"_NET_WM_NAME") else {
            return window_titles;
        };
        let Ok(utf8_string_cookie) = conn.intern_atom(false, b"UTF8_STRING") else {
            return window_titles;
        };
        let Ok(net_client_list_cookie) = conn.intern_atom(false, b"_NET_CLIENT_LIST") else {
            return window_titles;
        };

        let Ok(net_wm_pid_reply) = net_wm_pid_cookie.reply() else {
            return window_titles;
        };
        let Ok(net_wm_name_reply) = net_wm_name_cookie.reply() else {
            return window_titles;
        };
        let Ok(utf8_string_reply) = utf8_string_cookie.reply() else {
            return window_titles;
        };
        let Ok(net_client_list_reply) = net_client_list_cookie.reply() else {
            return window_titles;
        };

        let net_wm_pid = net_wm_pid_reply.atom;
        let net_wm_name = net_wm_name_reply.atom;
        let utf8_string = utf8_string_reply.atom;
        let net_client_list = net_client_list_reply.atom;

        // Get list of all client windows from the root window
        let Ok(client_list_cookie) =
            conn.get_property(false, root, net_client_list, AtomEnum::WINDOW, 0, u32::MAX)
        else {
            return window_titles;
        };

        let Ok(client_list_reply) = client_list_cookie.reply() else {
            return window_titles;
        };

        // Parse window IDs from the property value (array of 32-bit window IDs)
        let window_ids: Vec<u32> = client_list_reply
            .value
            .chunks_exact(4)
            .map(|chunk| u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();

        // For each window, get its PID and title
        for window_id in window_ids {
            // Get the window's PID
            let pid = Self::get_x11_window_pid(&conn, window_id, net_wm_pid);
            let Some(pid) = pid else {
                continue;
            };

            // Get the window's title (try _NET_WM_NAME first, fall back to WM_NAME)
            let title = Self::get_x11_window_title(&conn, window_id, net_wm_name, utf8_string);

            if let Some(title) = title {
                // Only store the first window title for each PID
                window_titles.entry(pid).or_insert(title);
            }
        }

        window_titles
    }

    /// Get the PID associated with an X11 window via _NET_WM_PID property.
    #[cfg(target_os = "linux")]
    fn get_x11_window_pid(
        conn: &impl x11rb::connection::Connection,
        window: u32,
        net_wm_pid: x11rb::protocol::xproto::Atom,
    ) -> Option<u32> {
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as XprotoConnectionExt};

        let pid_cookie = conn
            .get_property(false, window, net_wm_pid, AtomEnum::CARDINAL, 0, 1)
            .ok()?;
        let pid_reply = pid_cookie.reply().ok()?;

        if pid_reply.value.len() >= 4 {
            Some(u32::from_ne_bytes([
                pid_reply.value[0],
                pid_reply.value[1],
                pid_reply.value[2],
                pid_reply.value[3],
            ]))
        } else {
            None
        }
    }

    /// Get the title of an X11 window via _NET_WM_NAME or WM_NAME property.
    #[cfg(target_os = "linux")]
    fn get_x11_window_title(
        conn: &impl x11rb::connection::Connection,
        window: u32,
        net_wm_name: x11rb::protocol::xproto::Atom,
        utf8_string: x11rb::protocol::xproto::Atom,
    ) -> Option<String> {
        use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as XprotoConnectionExt};

        // Try _NET_WM_NAME first (UTF-8 encoded)
        let name_cookie = conn
            .get_property(false, window, net_wm_name, utf8_string, 0, u32::MAX)
            .ok()?;

        if let Ok(name_reply) = name_cookie.reply()
            && !name_reply.value.is_empty()
            && let Ok(title) = String::from_utf8(name_reply.value.clone())
            && !title.is_empty()
        {
            return Some(title);
        }

        // Fall back to WM_NAME (may be Latin-1 encoded)
        let wm_name_cookie = conn
            .get_property(
                false,
                window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                0,
                u32::MAX,
            )
            .ok()?;

        if let Ok(wm_name_reply) = wm_name_cookie.reply()
            && !wm_name_reply.value.is_empty()
        {
            // WM_NAME is typically Latin-1, but we try UTF-8 first
            if let Ok(title) = String::from_utf8(wm_name_reply.value.clone())
                && !title.is_empty()
            {
                return Some(title);
            }
            // Fall back to lossy conversion for Latin-1
            let title = String::from_utf8_lossy(&wm_name_reply.value).to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }

        None
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
        assert!(
            detector
                .config
                .process_definitions
                .contains_key(&SimId::Msfs)
        );
        assert!(
            detector
                .config
                .process_definitions
                .contains_key(&SimId::Msfs2024)
        );
        assert!(
            detector
                .config
                .process_definitions
                .contains_key(&SimId::XPlane)
        );
        assert!(
            detector
                .config
                .process_definitions
                .contains_key(&SimId::Dcs)
        );
        assert!(
            detector
                .config
                .process_definitions
                .contains_key(&SimId::AceCombat7)
        );
        assert!(
            detector
                .config
                .process_definitions
                .contains_key(&SimId::Ksp)
        );
    }

    #[test]
    fn test_process_definition_defaults() {
        let config = ProcessDetectionConfig::default();

        let msfs_def = config.process_definitions.get(&SimId::Msfs).unwrap();
        assert!(
            msfs_def
                .process_names
                .contains(&"FlightSimulator.exe".to_string())
        );
        assert!(
            msfs_def
                .window_titles
                .contains(&"Microsoft Flight Simulator".to_string())
        );
        assert_eq!(msfs_def.min_confidence, 0.8);

        let msfs2024_def = config.process_definitions.get(&SimId::Msfs2024).unwrap();
        assert!(
            msfs2024_def
                .process_names
                .contains(&"FlightSimulator2024.exe".to_string())
        );
        assert!(
            msfs2024_def
                .window_titles
                .contains(&"Microsoft Flight Simulator 2024".to_string())
        );
        assert_eq!(msfs2024_def.min_confidence, 0.8);

        let xplane_def = config.process_definitions.get(&SimId::XPlane).unwrap();
        assert!(
            xplane_def
                .process_names
                .contains(&"X-Plane.exe".to_string())
        );

        let dcs_def = config.process_definitions.get(&SimId::Dcs).unwrap();
        assert!(dcs_def.process_names.contains(&"DCS.exe".to_string()));

        let ac7_def = config.process_definitions.get(&SimId::AceCombat7).unwrap();
        assert!(
            ac7_def
                .process_names
                .contains(&"acecombat7.exe".to_string())
        );
    }

    #[tokio::test]
    async fn test_process_detection_lifecycle() {
        let config = ProcessDetectionConfig::default();
        let detector = Arc::new(ProcessDetector::new(config));

        // Should start successfully
        assert!(Arc::clone(&detector).start().await.is_ok());

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
        assert!(!detector.is_sim_detected(SimId::AceCombat7).await);
    }

    use proptest::prelude::*;

    proptest! {
        // Verify that process matching logic works correctly with various inputs
        #[test]
        fn prop_check_simulator_processes_match(
            name_fragment in "[a-zA-Z0-9]+",
            other_fragment in "[a-zA-Z0-9]+"
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();

            rt.block_on(async {
                let definition = ProcessDefinition {
                    process_names: vec![format!("{}.exe", name_fragment)],
                    window_titles: vec![],
                    process_paths: vec![],
                    min_confidence: 0.5,
                };

                // Exact match case
                let processes = vec![SystemProcess {
                    pid: 123,
                    name: format!("{}.exe", name_fragment),
                    path: PathBuf::from("C:\\test\\path"),
                    window_title: None,
                }];

                let detected = ProcessDetector::check_simulator_processes(
                    SimId::Msfs,
                    &definition,
                    &processes,
                    false
                ).await.unwrap();

                prop_assert!(detected.is_some());
                if let Some(d) = detected {
                    prop_assert!(d.confidence >= 0.6);
                }

                // Non-match case
                let expected_name = format!("{}.exe", name_fragment).to_lowercase();
                let candidate_name = format!("{}.exe", other_fragment).to_lowercase();
                if name_fragment != other_fragment && !candidate_name.contains(&expected_name) {
                    let processes = vec![SystemProcess {
                        pid: 123,
                        name: candidate_name,
                        path: PathBuf::from("C:\\test\\path"),
                        window_title: None,
                    }];

                    let detected = ProcessDetector::check_simulator_processes(
                        SimId::Msfs,
                        &definition,
                        &processes,
                        false
                    ).await.unwrap();

                    prop_assert!(detected.is_none());
                }

                Ok(())
            });
        }
    }

    #[test]
    fn ksp_definition_present() {
        let config = ProcessDetectionConfig::default();
        let ksp_def = config.process_definitions.get(&SimId::Ksp).unwrap();
        assert!(
            ksp_def.process_names.contains(&"KSP_x64.exe".to_string()),
            "Should contain Windows KSP executable"
        );
        assert!(
            ksp_def.process_names.contains(&"KSP.x86_64".to_string()),
            "Should contain Linux KSP executable"
        );
        assert!(
            ksp_def
                .window_titles
                .contains(&"Kerbal Space Program".to_string())
        );
        assert_eq!(ksp_def.min_confidence, 0.8);
    }

    #[test]
    fn wingman_definition_present() {
        let config = ProcessDetectionConfig::default();
        let def = config.process_definitions.get(&SimId::Wingman).unwrap();
        assert!(
            def.process_names
                .contains(&"ProjectWingman.exe".to_string()),
            "Should contain Wingman Windows executable"
        );
        assert!(def.window_titles.contains(&"Project Wingman".to_string()));
        assert_eq!(def.min_confidence, 0.7);
    }
} // end mod tests
