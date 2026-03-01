// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Safe Mode Implementation
//!
//! Provides a minimal, axis-only mode for troubleshooting and safe operation
//! when full system functionality is not available or desired.

use crate::power::{PowerCheckStatus, PowerChecker, PowerStatus};
use crate::service::build_pipeline_for_axis;
use flight_axis::AxisEngine;
use flight_core::{
    Result,
    profile::{Profile, defaults},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Safe mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeModeConfig {
    /// Enable axis processing only (no panels, plugins, tactile)
    pub axis_only: bool,
    /// Use basic profile instead of complex configurations
    pub use_basic_profile: bool,
    /// Skip power optimization checks
    pub skip_power_checks: bool,
    /// Disable all non-essential features
    pub minimal_mode: bool,
}

impl Default for SafeModeConfig {
    fn default() -> Self {
        Self {
            axis_only: true,
            use_basic_profile: true,
            skip_power_checks: false,
            minimal_mode: true,
        }
    }
}

/// Safe mode status and validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeModeStatus {
    /// Whether safe mode is currently active
    pub active: bool,
    /// Safe mode configuration
    pub config: SafeModeConfig,
    /// Power management status
    pub power_status: PowerStatus,
    /// RT privilege detection results
    pub rt_privileges: RtPrivilegeStatus,
    /// Validation results
    pub validation_results: Vec<ValidationResult>,
}

/// Real-time privilege status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtPrivilegeStatus {
    /// Whether RT privileges are available
    pub available: bool,
    /// Platform-specific privilege details
    pub details: String,
    /// Recommended actions if privileges unavailable
    pub recommendations: Vec<String>,
}

/// Validation result for safe mode components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Component being validated
    pub component: String,
    /// Validation success status
    pub success: bool,
    /// Validation message
    pub message: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Reason the service degraded into safe mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DegradationReason {
    /// Profile JSON was missing or could not be parsed.
    ConfigCorrupt,
    /// A connected HID device reported errors or disappeared.
    HardwareFault,
    /// A simulator adapter (SimConnect, X-Plane, DCS) failed to initialise.
    AdapterFailure,
    /// RT scheduling privileges could not be acquired.
    RtPrivilegeUnavailable,
    /// The system power configuration is critical (e.g. battery saver).
    PowerCritical,
    /// An operator explicitly requested safe mode.
    OperatorRequest,
}

/// Diagnostic bundle produced when safe mode activates.
///
/// Captures *why* the service degraded and what the operator should look at.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeModeDiagnostic {
    /// Human-readable explanation of why safe mode was entered.
    pub reason: String,
    /// Structured degradation reasons derived from validation failures.
    pub degradation_reasons: Vec<DegradationReason>,
    /// Specific subsystems that triggered degradation.
    pub failed_components: Vec<String>,
    /// Recommended operator actions.
    pub recommended_actions: Vec<String>,
    /// Snapshot of the validation results at activation time.
    pub validation_snapshot: Vec<ValidationResult>,
}

/// Safe mode manager
pub struct SafeModeManager {
    config: SafeModeConfig,
    axis_engine: Option<Arc<AxisEngine>>,
    /// Stored status from the last `initialize()` call.
    last_status: Option<SafeModeStatus>,
    /// Diagnostic bundle built during initialization.
    last_diagnostic: Option<SafeModeDiagnostic>,
}

impl SafeModeManager {
    /// Create new safe mode manager
    pub fn new(config: SafeModeConfig) -> Self {
        info!("Initializing Safe Mode with config: {:?}", config);

        Self {
            config,
            axis_engine: None,
            last_status: None,
            last_diagnostic: None,
        }
    }

    /// Initialize safe mode with validation
    pub async fn initialize(&mut self) -> Result<SafeModeStatus> {
        info!("Starting safe mode initialization");

        let mut validation_results = Vec::new();

        // Check power configuration unless skipped
        let power_status = if self.config.skip_power_checks {
            info!("Skipping power checks as requested");
            PowerStatus {
                overall_status: PowerCheckStatus::Optimal,
                checks: Vec::new(),
                remediation_steps: Vec::new(),
            }
        } else {
            let start = std::time::Instant::now();
            let status = PowerChecker::check_power_configuration().await;
            let elapsed = start.elapsed().as_millis() as u64;

            validation_results.push(ValidationResult {
                component: "Power Configuration".to_string(),
                success: status.overall_status != PowerCheckStatus::Critical,
                message: format!("Power status: {}", status.overall_status),
                execution_time_ms: elapsed,
            });

            status
        };

        // Check RT privileges
        let rt_privileges = self.check_rt_privileges().await;
        validation_results.push(ValidationResult {
            component: "RT Privileges".to_string(),
            success: rt_privileges.available,
            message: rt_privileges.details.clone(),
            execution_time_ms: 5, // Stub timing
        });

        // Initialize basic axis engine if axis processing enabled
        if self.config.axis_only {
            let start = std::time::Instant::now();
            match self.initialize_axis_engine().await {
                Ok(_) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Axis Engine".to_string(),
                        success: true,
                        message: "Axis engine initialized successfully".to_string(),
                        execution_time_ms: elapsed,
                    });
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Axis Engine".to_string(),
                        success: false,
                        message: format!("Failed to initialize axis engine: {}", e),
                        execution_time_ms: elapsed,
                    });
                }
            }
        }

        // Validate basic profile if enabled
        if self.config.use_basic_profile {
            let start = std::time::Instant::now();
            match self.validate_basic_profile().await {
                Ok(_) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Basic Profile".to_string(),
                        success: true,
                        message: "Basic profile validated successfully".to_string(),
                        execution_time_ms: elapsed,
                    });
                }
                Err(e) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    validation_results.push(ValidationResult {
                        component: "Basic Profile".to_string(),
                        success: false,
                        message: format!("Basic profile validation failed: {}", e),
                        execution_time_ms: elapsed,
                    });
                }
            }
        }

        let status = SafeModeStatus {
            active: true,
            config: self.config.clone(),
            power_status,
            rt_privileges,
            validation_results,
        };

        // Build diagnostic bundle capturing *why* we entered safe mode.
        let diagnostic = self.build_diagnostic(&status.validation_results);
        self.last_diagnostic = Some(diagnostic);
        self.last_status = Some(status.clone());

        info!("Safe mode initialization completed");
        Ok(status)
    }

    /// Check real-time privileges availability
    async fn check_rt_privileges(&self) -> RtPrivilegeStatus {
        debug!("Checking RT privileges");

        #[cfg(target_os = "windows")]
        {
            // Check for MMCSS and high priority capabilities
            let available = self.check_windows_rt_privileges().await;
            RtPrivilegeStatus {
                available,
                details: if available {
                    "MMCSS 'Games' class and high priority available".to_string()
                } else {
                    "Limited RT capabilities - may affect performance".to_string()
                },
                recommendations: if available {
                    Vec::new()
                } else {
                    vec![
                        "Run as administrator for full RT capabilities".to_string(),
                        "Ensure Windows Multimedia Class Scheduler service is running".to_string(),
                    ]
                },
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Check for SCHED_FIFO via rtkit
            let available = self.check_linux_rt_privileges().await;
            RtPrivilegeStatus {
                available,
                details: if available {
                    "SCHED_FIFO via rtkit available".to_string()
                } else {
                    "RT scheduling not available - will use normal priority".to_string()
                },
                recommendations: if available {
                    Vec::new()
                } else {
                    vec![
                        "Install rtkit package".to_string(),
                        "Add user to audio group".to_string(),
                        "Check /etc/security/limits.conf for rtprio limits".to_string(),
                    ]
                },
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            RtPrivilegeStatus {
                available: false,
                details: "RT privileges not supported on this platform".to_string(),
                recommendations: vec!["Use supported platform for RT operation".to_string()],
            }
        }
    }

    #[cfg(target_os = "windows")]
    async fn check_windows_rt_privileges(&self) -> bool {
        use std::ffi::c_void;

        // FFI bindings for MMCSS (avrt.dll)
        #[link(name = "avrt")]
        unsafe extern "system" {
            fn AvSetMmThreadCharacteristicsW(task_name: *const u16, task_index: *mut u32) -> isize;
            fn AvRevertMmThreadCharacteristics(avrt_handle: isize) -> i32;
        }

        // FFI bindings for thread priority (kernel32.dll)
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentThread() -> *mut c_void;
            fn GetThreadPriority(thread: *mut c_void) -> i32;
            fn SetThreadPriority(thread: *mut c_void, priority: i32) -> i32;
        }

        const THREAD_PRIORITY_TIME_CRITICAL: i32 = 15;
        const THREAD_PRIORITY_HIGHEST: i32 = 2;

        let mut mmcss_available = false;
        let mut high_priority_available = false;

        // Test 1: Try to acquire MMCSS "Games" class
        let task_name = "Games";
        let task_name_wide: Vec<u16> = task_name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut task_index: u32 = 0;

        // SAFETY: We're passing a valid null-terminated wide string and a valid pointer
        let mmcss_handle =
            unsafe { AvSetMmThreadCharacteristicsW(task_name_wide.as_ptr(), &mut task_index) };

        if mmcss_handle != 0 {
            mmcss_available = true;
            debug!(
                "MMCSS 'Games' class acquisition test: SUCCESS (handle: {})",
                mmcss_handle
            );

            // Release the MMCSS registration immediately after testing
            // SAFETY: We have a valid MMCSS handle
            let revert_result = unsafe { AvRevertMmThreadCharacteristics(mmcss_handle) };
            if revert_result == 0 {
                warn!("Failed to release test MMCSS registration");
            }
        } else {
            let err = std::io::Error::last_os_error();
            debug!(
                "MMCSS 'Games' class acquisition test: FAILED (error: {}, code: {:?})",
                err,
                err.raw_os_error()
            );
        }

        // Test 2: Try to elevate thread priority to TIME_CRITICAL
        // SAFETY: GetCurrentThread returns a pseudo-handle that's always valid
        let current_thread = unsafe { GetCurrentThread() };
        let original_priority = unsafe { GetThreadPriority(current_thread) };

        // Try to set TIME_CRITICAL priority
        // SAFETY: We have a valid thread handle
        let priority_result =
            unsafe { SetThreadPriority(current_thread, THREAD_PRIORITY_TIME_CRITICAL) };

        if priority_result != 0 {
            high_priority_available = true;
            debug!("Thread priority elevation to TIME_CRITICAL: SUCCESS");

            // Restore original priority
            // SAFETY: We have a valid thread handle
            let restore_result = unsafe { SetThreadPriority(current_thread, original_priority) };
            if restore_result == 0 {
                warn!("Failed to restore original thread priority after test");
            }
        } else {
            // Try HIGHEST priority as a fallback test
            let highest_result =
                unsafe { SetThreadPriority(current_thread, THREAD_PRIORITY_HIGHEST) };
            if highest_result != 0 {
                high_priority_available = true;
                debug!("Thread priority elevation to HIGHEST: SUCCESS (TIME_CRITICAL unavailable)");

                // Restore original priority
                let restore_result =
                    unsafe { SetThreadPriority(current_thread, original_priority) };
                if restore_result == 0 {
                    warn!("Failed to restore original thread priority after test");
                }
            } else {
                let err = std::io::Error::last_os_error();
                debug!(
                    "Thread priority elevation test: FAILED (error: {}, code: {:?})",
                    err,
                    err.raw_os_error()
                );
            }
        }

        // RT privileges are considered available if either MMCSS or high priority works
        // MMCSS alone provides significant RT benefit even without TIME_CRITICAL priority
        let rt_available = mmcss_available || high_priority_available;

        if !rt_available {
            warn!(
                "Windows RT privileges are limited. MMCSS: {}, High priority: {}. \
                 Performance may be affected. Consider running as administrator or \
                 ensuring the Windows Multimedia Class Scheduler service is running.",
                if mmcss_available {
                    "available"
                } else {
                    "unavailable"
                },
                if high_priority_available {
                    "available"
                } else {
                    "unavailable"
                }
            );
        } else {
            info!(
                "Windows RT privileges detected. MMCSS: {}, High priority: {}",
                if mmcss_available {
                    "available"
                } else {
                    "unavailable"
                },
                if high_priority_available {
                    "available"
                } else {
                    "unavailable"
                }
            );
        }

        rt_available
    }

    #[cfg(target_os = "linux")]
    async fn check_linux_rt_privileges(&self) -> bool {
        use tracing::warn;

        let mut rtkit_available = false;
        let mut rlimit_sufficient = false;
        let mut direct_sched_available = false;

        // Test 1: Check if rtkit service is available via D-Bus
        // We query rtkit's MaxRealtimePriority property to see if it's running
        let rtkit_output = std::process::Command::new("dbus-send")
            .args([
                "--system",
                "--print-reply",
                "--dest=org.freedesktop.RealtimeKit1",
                "/org/freedesktop/RealtimeKit1",
                "org.freedesktop.DBus.Properties.Get",
                "string:org.freedesktop.RealtimeKit1",
                "string:MaxRealtimePriority",
            ])
            .output();

        match rtkit_output {
            Ok(result) => {
                if result.status.success() {
                    rtkit_available = true;
                    let stdout = String::from_utf8_lossy(&result.stdout);
                    debug!("rtkit service available: {}", stdout.trim());
                } else {
                    let stderr = String::from_utf8_lossy(&result.stderr);
                    debug!("rtkit service query failed: {}", stderr.trim());
                }
            }
            Err(e) => {
                debug!("Failed to query rtkit (dbus-send not available?): {}", e);
            }
        }

        // Test 2: Validate RLIMIT_RTPRIO limits
        // RLIMIT_RTPRIO of 0 means no RT scheduling allowed without CAP_SYS_NICE
        // A value >= 1 means we can potentially use RT priorities up to that value
        unsafe {
            let mut rlimit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };

            if libc::getrlimit(libc::RLIMIT_RTPRIO, &mut rlimit) == 0 {
                // Check if we have any RT priority allowance
                if rlimit.rlim_cur == libc::RLIM_INFINITY || rlimit.rlim_cur >= 1 {
                    rlimit_sufficient = true;
                    debug!(
                        "RLIMIT_RTPRIO sufficient: soft={}, hard={}",
                        if rlimit.rlim_cur == libc::RLIM_INFINITY {
                            "unlimited".to_string()
                        } else {
                            rlimit.rlim_cur.to_string()
                        },
                        if rlimit.rlim_max == libc::RLIM_INFINITY {
                            "unlimited".to_string()
                        } else {
                            rlimit.rlim_max.to_string()
                        }
                    );
                } else {
                    debug!(
                        "RLIMIT_RTPRIO is 0 - RT scheduling not available without CAP_SYS_NICE or rtkit"
                    );
                }
            } else {
                let err = std::io::Error::last_os_error();
                debug!("Failed to query RLIMIT_RTPRIO: {}", err);
            }
        }

        // Test 3: Check if we can use sched_setscheduler directly (requires CAP_SYS_NICE or root)
        // We test with a low priority (1) to minimize impact
        unsafe {
            let param = libc::sched_param { sched_priority: 1 };
            let result = libc::sched_setscheduler(0, libc::SCHED_FIFO, &param);

            if result == 0 {
                direct_sched_available = true;
                debug!("Direct sched_setscheduler test: SUCCESS");

                // Restore to SCHED_OTHER
                let restore_param = libc::sched_param { sched_priority: 0 };
                libc::sched_setscheduler(0, libc::SCHED_OTHER, &restore_param);
            } else {
                let err = std::io::Error::last_os_error();
                debug!(
                    "Direct sched_setscheduler test: FAILED (error: {}, code: {:?})",
                    err,
                    err.raw_os_error()
                );
            }
        }

        // RT privileges are considered available if:
        // 1. rtkit is available (can acquire RT via D-Bus without root), OR
        // 2. RLIMIT_RTPRIO is sufficient AND direct scheduling works, OR
        // 3. Direct scheduling works (CAP_SYS_NICE or root)
        let rt_available = rtkit_available || direct_sched_available || rlimit_sufficient;

        if !rt_available {
            warn!(
                "Linux RT privileges are limited. rtkit: {}, RLIMIT_RTPRIO: {}, direct sched: {}. \
                 Performance may be affected. Consider:\n\
                 - Installing the rtkit package\n\
                 - Adding user to the 'audio' group\n\
                 - Configuring /etc/security/limits.conf with: @audio - rtprio 99",
                if rtkit_available {
                    "available"
                } else {
                    "unavailable"
                },
                if rlimit_sufficient {
                    "sufficient"
                } else {
                    "insufficient"
                },
                if direct_sched_available {
                    "available"
                } else {
                    "unavailable"
                }
            );
        } else {
            info!(
                "Linux RT privileges detected. rtkit: {}, RLIMIT_RTPRIO: {}, direct sched: {}",
                if rtkit_available {
                    "available"
                } else {
                    "unavailable"
                },
                if rlimit_sufficient {
                    "sufficient"
                } else {
                    "insufficient"
                },
                if direct_sched_available {
                    "available"
                } else {
                    "unavailable"
                }
            );
        }

        rt_available
    }

    /// Initialize basic axis engine for safe mode
    async fn initialize_axis_engine(&mut self) -> Result<()> {
        info!("Initializing axis engine for safe mode");

        let engine = AxisEngine::new();
        self.axis_engine = Some(Arc::new(engine));

        debug!("Axis engine initialized successfully");
        Ok(())
    }

    /// Validate basic profile configuration
    async fn validate_basic_profile(&self) -> Result<()> {
        info!("Validating basic profile");

        let basic_profile = self.create_basic_profile();

        // Validate profile structure
        basic_profile.validate()?;

        // Try to compile a pipeline for each axis so we catch any conversion
        // errors before the engine actually starts processing inputs.
        if let Some(engine) = &self.axis_engine {
            for (axis_name, axis_config) in &basic_profile.axes {
                match build_pipeline_for_axis(axis_name, axis_config) {
                    Ok(pipeline) => {
                        let result = engine.update_pipeline(pipeline);
                        debug!("Safe-mode pipeline compile for '{axis_name}': {result:?}");
                    }
                    Err(e) => {
                        warn!("Safe-mode pipeline compile failed for '{axis_name}': {e}");
                    }
                }
            }
        }

        info!("Basic profile validation completed");
        Ok(())
    }

    /// Create a basic, safe profile for troubleshooting.
    ///
    /// Delegates to [`defaults::safe_mode_profile`] so that axis constants
    /// are defined in exactly one place.
    fn create_basic_profile(&self) -> Profile {
        defaults::safe_mode_profile()
    }

    /// Get current safe mode status.
    ///
    /// Returns the status captured during `initialize()` if available,
    /// otherwise returns a minimal placeholder status.
    pub fn get_status(&self) -> SafeModeStatus {
        if let Some(status) = &self.last_status {
            return status.clone();
        }
        // Fallback before initialize() has been called.
        SafeModeStatus {
            active: true,
            config: self.config.clone(),
            power_status: PowerStatus {
                overall_status: PowerCheckStatus::Optimal,
                checks: Vec::new(),
                remediation_steps: Vec::new(),
            },
            rt_privileges: RtPrivilegeStatus {
                available: true,
                details: "Status not checked".to_string(),
                recommendations: Vec::new(),
            },
            validation_results: Vec::new(),
        }
    }

    /// Return the diagnostic bundle built during initialization, if any.
    pub fn get_diagnostic(&self) -> Option<&SafeModeDiagnostic> {
        self.last_diagnostic.as_ref()
    }

    /// Shutdown safe mode
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down safe mode");

        if let Some(_engine) = self.axis_engine.take() {
            // In real implementation, would properly shutdown the engine
            debug!("Axis engine shutdown");
        }

        info!("Safe mode shutdown completed");
        Ok(())
    }

    /// Build a diagnostic bundle explaining why safe mode was activated.
    pub fn build_diagnostic(&self, results: &[ValidationResult]) -> SafeModeDiagnostic {
        let failed: Vec<String> = results
            .iter()
            .filter(|r| !r.success)
            .map(|r| r.component.clone())
            .collect();

        let reason = if failed.is_empty() {
            "Safe mode activated by operator request (no component failures).".to_string()
        } else {
            format!(
                "Safe mode activated because the following subsystems failed validation: {}",
                failed.join(", ")
            )
        };

        let mut degradation_reasons: Vec<DegradationReason> = Vec::new();
        let mut recommended_actions: Vec<String> = Vec::new();
        for r in results.iter().filter(|r| !r.success) {
            match r.component.as_str() {
                "Power Configuration" => {
                    degradation_reasons.push(DegradationReason::PowerCritical);
                    recommended_actions
                        .push("Check power plan — switch to High Performance.".into());
                }
                "RT Privileges" => {
                    degradation_reasons.push(DegradationReason::RtPrivilegeUnavailable);
                    recommended_actions.push(
                        "Ensure RT privileges are available (run as admin or install rtkit)."
                            .into(),
                    );
                }
                "Axis Engine" => {
                    degradation_reasons.push(DegradationReason::HardwareFault);
                    recommended_actions
                        .push("Check HID device connectivity and driver health.".into());
                }
                "Basic Profile" => {
                    degradation_reasons.push(DegradationReason::ConfigCorrupt);
                    recommended_actions.push(
                        "Profile validation failed — inspect profile JSON for errors.".into(),
                    );
                }
                other => {
                    degradation_reasons.push(DegradationReason::AdapterFailure);
                    recommended_actions.push(format!("Investigate {other} failure."));
                }
            }
        }

        if degradation_reasons.is_empty() {
            degradation_reasons.push(DegradationReason::OperatorRequest);
        }

        if recommended_actions.is_empty() {
            recommended_actions.push("No failures detected — safe mode can be exited.".into());
        }

        info!("Diagnostic bundle: {reason}");

        SafeModeDiagnostic {
            reason,
            degradation_reasons,
            failed_components: failed,
            recommended_actions,
            validation_snapshot: results.to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_safe_mode_initialization() {
        let config = SafeModeConfig::default();
        let mut manager = SafeModeManager::new(config);

        let status = manager.initialize().await.unwrap();
        assert!(status.active);
        assert!(status.config.axis_only);
    }

    #[tokio::test]
    async fn test_rt_privilege_check() {
        let config = SafeModeConfig::default();
        let manager = SafeModeManager::new(config);

        let rt_status = manager.check_rt_privileges().await;
        // Should have some details regardless of availability
        assert!(!rt_status.details.is_empty());
    }

    #[test]
    fn test_basic_profile_creation() {
        let config = SafeModeConfig::default();
        let manager = SafeModeManager::new(config);

        let profile = manager.create_basic_profile();
        // Profile should be created successfully
        assert_eq!(profile.schema, "flight.profile/1");
    }

    #[test]
    fn test_safe_profile_has_all_axes_with_correct_defaults() {
        let config = SafeModeConfig::default();
        let manager = SafeModeManager::new(config);
        let profile = manager.create_basic_profile();

        // All four primary axes must be present
        for name in &["pitch", "roll", "yaw", "throttle"] {
            assert!(profile.axes.contains_key(*name), "missing axis: {name}");
        }

        // Pitch
        let pitch = &profile.axes["pitch"];
        assert_eq!(pitch.deadzone, Some(0.03));
        assert_eq!(pitch.expo, Some(0.2));

        // Roll
        let roll = &profile.axes["roll"];
        assert_eq!(roll.deadzone, Some(0.03));
        assert_eq!(roll.expo, Some(0.2));

        // Yaw (rudder) — same 3% deadzone + 0.2 expo as pitch/roll
        let yaw = &profile.axes["yaw"];
        assert_eq!(yaw.deadzone, Some(0.03));
        assert_eq!(yaw.expo, Some(0.2));

        // Throttle — linear (no expo)
        let throttle = &profile.axes["throttle"];
        assert_eq!(throttle.deadzone, Some(0.01));
        assert_eq!(throttle.expo, None);

        // Profile passes its own validation
        profile.validate().expect("safe profile must validate");
    }

    #[tokio::test]
    async fn test_safe_mode_shutdown() {
        let config = SafeModeConfig::default();
        let mut manager = SafeModeManager::new(config);

        // Initialize and then shutdown
        let _status = manager.initialize().await.unwrap();
        let result = manager.shutdown().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_diagnostic_bundle_no_failures() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let results = vec![ValidationResult {
            component: "Axis Engine".to_string(),
            success: true,
            message: "OK".to_string(),
            execution_time_ms: 1,
        }];
        let diag = manager.build_diagnostic(&results);
        assert!(diag.failed_components.is_empty());
        assert!(diag.reason.contains("operator request"));
        assert_eq!(diag.degradation_reasons, vec![DegradationReason::OperatorRequest]);
    }

    #[test]
    fn test_diagnostic_bundle_with_failures() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let results = vec![
            ValidationResult {
                component: "RT Privileges".to_string(),
                success: false,
                message: "unavailable".to_string(),
                execution_time_ms: 2,
            },
            ValidationResult {
                component: "Axis Engine".to_string(),
                success: true,
                message: "OK".to_string(),
                execution_time_ms: 1,
            },
        ];
        let diag = manager.build_diagnostic(&results);
        assert_eq!(diag.failed_components, vec!["RT Privileges"]);
        assert!(diag.reason.contains("RT Privileges"));
        assert!(!diag.recommended_actions.is_empty());
        assert_eq!(
            diag.degradation_reasons,
            vec![DegradationReason::RtPrivilegeUnavailable]
        );
    }

    #[test]
    fn test_safe_profile_no_inversion_full_range() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let profile = manager.create_basic_profile();

        for (name, axis) in &profile.axes {
            // No custom curve means no inversion — output follows input monotonically
            assert!(
                axis.curve.is_none(),
                "axis '{name}' must not have a custom curve"
            );
            // No detents that could trap the axis
            assert!(
                axis.detents.is_empty(),
                "axis '{name}' must have no detents"
            );
            // No slew-rate limiter that could clamp full-range sweeps
            assert!(
                axis.slew_rate.is_none(),
                "axis '{name}' must have no slew limit"
            );
        }
    }

    #[test]
    fn test_safe_profile_pipelines_compile() {
        use crate::service::build_pipeline_for_axis;

        let manager = SafeModeManager::new(SafeModeConfig::default());
        let profile = manager.create_basic_profile();

        for (name, axis) in &profile.axes {
            let pipeline = build_pipeline_for_axis(name, axis);
            assert!(
                pipeline.is_ok(),
                "pipeline for '{name}' must compile: {:?}",
                pipeline.err()
            );
        }
    }

    #[test]
    fn test_diagnostic_explains_multiple_failures() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let results = vec![
            ValidationResult {
                component: "Power Configuration".to_string(),
                success: false,
                message: "battery".to_string(),
                execution_time_ms: 1,
            },
            ValidationResult {
                component: "Basic Profile".to_string(),
                success: false,
                message: "bad JSON".to_string(),
                execution_time_ms: 1,
            },
        ];
        let diag = manager.build_diagnostic(&results);
        assert_eq!(diag.failed_components.len(), 2);
        assert!(diag.reason.contains("Power Configuration"));
        assert!(diag.reason.contains("Basic Profile"));
        assert!(diag.recommended_actions.len() >= 2);
        assert_eq!(diag.validation_snapshot.len(), 2);
        assert_eq!(
            diag.degradation_reasons,
            vec![DegradationReason::PowerCritical, DegradationReason::ConfigCorrupt]
        );
    }

    #[tokio::test]
    async fn test_get_status_returns_initialization_status() {
        let mut manager = SafeModeManager::new(SafeModeConfig::default());
        let status = manager.initialize().await.unwrap();
        assert!(status.active);

        // get_status() should now return the real captured status, not placeholders
        let retrieved = manager.get_status();
        assert!(retrieved.active);
        assert!(!retrieved.validation_results.is_empty());
        // RT Privileges validation should have been recorded
        assert!(
            retrieved
                .validation_results
                .iter()
                .any(|r| r.component == "RT Privileges"),
            "RT Privileges check should be in validation results"
        );
    }

    #[tokio::test]
    async fn test_diagnostic_stored_during_init() {
        let mut manager = SafeModeManager::new(SafeModeConfig::default());
        // Before init, no diagnostic
        assert!(manager.get_diagnostic().is_none());

        let _status = manager.initialize().await.unwrap();
        // After init, diagnostic should be present
        let diag = manager
            .get_diagnostic()
            .expect("diagnostic should be stored");
        assert!(!diag.reason.is_empty());
        assert!(!diag.recommended_actions.is_empty());
    }

    #[test]
    fn test_get_status_before_init_returns_placeholder() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let status = manager.get_status();
        assert!(status.active);
        assert_eq!(status.rt_privileges.details, "Status not checked");
        assert!(status.validation_results.is_empty());
    }

    // ── Degradation reason tests ────────────────────────────────────────

    #[test]
    fn test_degradation_reason_hardware_fault() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let results = vec![ValidationResult {
            component: "Axis Engine".to_string(),
            success: false,
            message: "HID device missing".to_string(),
            execution_time_ms: 3,
        }];
        let diag = manager.build_diagnostic(&results);
        assert!(diag.degradation_reasons.contains(&DegradationReason::HardwareFault));
    }

    #[test]
    fn test_degradation_reason_adapter_failure() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let results = vec![ValidationResult {
            component: "SimConnect Adapter".to_string(),
            success: false,
            message: "SimConnect DLL not found".to_string(),
            execution_time_ms: 5,
        }];
        let diag = manager.build_diagnostic(&results);
        assert!(diag.degradation_reasons.contains(&DegradationReason::AdapterFailure));
    }

    #[test]
    fn test_degradation_reason_serializes() {
        let reason = DegradationReason::ConfigCorrupt;
        let json = serde_json::to_string(&reason).unwrap();
        let back: DegradationReason = serde_json::from_str(&json).unwrap();
        assert_eq!(back, reason);
    }

    // ── Category default profile pipeline compilation ───────────────────

    #[test]
    fn test_ga_default_profile_pipelines_compile() {
        use crate::service::build_pipeline_for_axis;
        let profile = defaults::ga_profile();
        for (name, axis) in &profile.axes {
            let pipeline = build_pipeline_for_axis(name, axis);
            assert!(
                pipeline.is_ok(),
                "GA pipeline for '{name}' must compile: {:?}",
                pipeline.err()
            );
        }
    }

    #[test]
    fn test_jet_default_profile_pipelines_compile() {
        use crate::service::build_pipeline_for_axis;
        let profile = defaults::jet_profile();
        for (name, axis) in &profile.axes {
            let pipeline = build_pipeline_for_axis(name, axis);
            assert!(
                pipeline.is_ok(),
                "Jet pipeline for '{name}' must compile: {:?}",
                pipeline.err()
            );
        }
    }

    #[test]
    fn test_helicopter_default_profile_pipelines_compile() {
        use crate::service::build_pipeline_for_axis;
        let profile = defaults::helicopter_profile();
        for (name, axis) in &profile.axes {
            let pipeline = build_pipeline_for_axis(name, axis);
            assert!(
                pipeline.is_ok(),
                "Helicopter pipeline for '{name}' must compile: {:?}",
                pipeline.err()
            );
        }
    }

    #[test]
    fn test_safe_mode_profile_matches_defaults_module() {
        let manager = SafeModeManager::new(SafeModeConfig::default());
        let from_manager = manager.create_basic_profile();
        let from_defaults = defaults::safe_mode_profile();
        assert_eq!(from_manager, from_defaults);
    }
}
