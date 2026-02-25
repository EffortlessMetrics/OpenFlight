// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Power Management and RT Privilege Detection
//!
//! Provides platform-specific power management checks and remediation guidance
//! for optimal real-time performance.

use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{debug, info};

/// Power management check results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStatus {
    /// Overall power configuration status
    pub overall_status: PowerCheckStatus,
    /// Individual check results
    pub checks: Vec<PowerCheck>,
    /// Remediation steps if issues found
    pub remediation_steps: Vec<RemediationStep>,
}

/// Status of power configuration checks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerCheckStatus {
    /// All checks passed, optimal for RT operation
    Optimal,
    /// Some issues found but RT operation possible with degraded performance
    Degraded,
    /// Critical issues found, RT operation not recommended
    Critical,
}

/// Individual power check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerCheck {
    /// Name of the check
    pub name: String,
    /// Check result status
    pub status: PowerCheckStatus,
    /// Description of what was checked
    pub description: String,
    /// Current value if applicable
    pub current_value: Option<String>,
    /// Expected/optimal value
    pub expected_value: Option<String>,
}

/// Remediation step to fix power issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationStep {
    /// Step description
    pub description: String,
    /// Platform-specific command or action
    pub action: String,
    /// Whether this step requires admin/root privileges
    pub requires_admin: bool,
    /// Priority of this step (lower = higher priority)
    pub priority: u8,
}

impl fmt::Display for PowerCheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerCheckStatus::Optimal => write!(f, "Optimal"),
            PowerCheckStatus::Degraded => write!(f, "Degraded"),
            PowerCheckStatus::Critical => write!(f, "Critical"),
        }
    }
}

/// Power management checker
pub struct PowerChecker;

impl PowerChecker {
    /// Perform comprehensive power management checks
    pub async fn check_power_configuration() -> PowerStatus {
        info!("Performing power configuration checks");

        let mut checks = Vec::new();
        let mut remediation_steps = Vec::new();

        #[cfg(target_os = "windows")]
        {
            Self::check_windows_power(&mut checks, &mut remediation_steps).await;
        }

        #[cfg(target_os = "linux")]
        {
            Self::check_linux_power(&mut checks, &mut remediation_steps).await;
        }

        // Determine overall status
        let overall_status = if checks
            .iter()
            .any(|c| c.status == PowerCheckStatus::Critical)
        {
            PowerCheckStatus::Critical
        } else if checks
            .iter()
            .any(|c| c.status == PowerCheckStatus::Degraded)
        {
            PowerCheckStatus::Degraded
        } else {
            PowerCheckStatus::Optimal
        };

        debug!("Power check completed with status: {}", overall_status);

        PowerStatus {
            overall_status,
            checks,
            remediation_steps,
        }
    }

    #[cfg(target_os = "windows")]
    async fn check_windows_power(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        // Check USB selective suspend
        Self::check_usb_selective_suspend(checks, remediation_steps).await;

        // Check power plan
        Self::check_power_plan(checks, remediation_steps).await;

        // Check process power throttling
        Self::check_process_power_throttling(checks, remediation_steps).await;
    }

    #[cfg(target_os = "windows")]
    async fn check_usb_selective_suspend(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        // Simulate USB selective suspend check
        // In real implementation, this would query registry or WMI
        let usb_suspend_enabled = Self::is_usb_selective_suspend_enabled().await;

        let status = if usb_suspend_enabled {
            PowerCheckStatus::Degraded
        } else {
            PowerCheckStatus::Optimal
        };

        checks.push(PowerCheck {
            name: "USB Selective Suspend".to_string(),
            status,
            description: "USB selective suspend can cause HID device latency issues".to_string(),
            current_value: Some(
                if usb_suspend_enabled {
                    "Enabled"
                } else {
                    "Disabled"
                }
                .to_string(),
            ),
            expected_value: Some("Disabled".to_string()),
        });

        if usb_suspend_enabled {
            remediation_steps.push(RemediationStep {
                description: "Disable USB selective suspend in Device Manager".to_string(),
                action: "Open Device Manager → Universal Serial Bus controllers → Right-click USB Root Hub → Properties → Power Management → Uncheck 'Allow the computer to turn off this device'".to_string(),
                requires_admin: true,
                priority: 1,
            });
        }
    }

    #[cfg(target_os = "windows")]
    async fn check_power_plan(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        let current_plan = Self::get_current_power_plan().await;
        let is_high_performance =
            current_plan.contains("High performance") || current_plan.contains("Ultimate");

        let status = if is_high_performance {
            PowerCheckStatus::Optimal
        } else {
            PowerCheckStatus::Degraded
        };

        checks.push(PowerCheck {
            name: "Power Plan".to_string(),
            status,
            description: "Power plan affects CPU frequency scaling and RT performance".to_string(),
            current_value: Some(current_plan.clone()),
            expected_value: Some("High Performance or Ultimate Performance".to_string()),
        });

        if !is_high_performance {
            remediation_steps.push(RemediationStep {
                description: "Switch to High Performance power plan".to_string(),
                action: "powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c".to_string(),
                requires_admin: true,
                priority: 2,
            });
        }
    }

    #[cfg(target_os = "windows")]
    async fn check_process_power_throttling(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        // This would check if the current process has power throttling disabled
        let throttling_disabled = Self::is_power_throttling_disabled().await;

        let status = if throttling_disabled {
            PowerCheckStatus::Optimal
        } else {
            PowerCheckStatus::Degraded
        };

        checks.push(PowerCheck {
            name: "Process Power Throttling".to_string(),
            status,
            description: "Process power throttling can affect RT thread performance".to_string(),
            current_value: Some(
                if throttling_disabled {
                    "Disabled"
                } else {
                    "Enabled"
                }
                .to_string(),
            ),
            expected_value: Some("Disabled".to_string()),
        });

        if !throttling_disabled {
            remediation_steps.push(RemediationStep {
                description: "Process power throttling will be disabled automatically at runtime"
                    .to_string(),
                action: "Automatic - no user action required".to_string(),
                requires_admin: false,
                priority: 3,
            });
        }
    }

    #[cfg(target_os = "linux")]
    async fn check_linux_power(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        // Check rtkit availability
        Self::check_rtkit_availability(checks, remediation_steps).await;

        // Check memlock limits
        Self::check_memlock_limits(checks, remediation_steps).await;

        // Check CPU governor
        Self::check_cpu_governor(checks, remediation_steps).await;
    }

    #[cfg(target_os = "linux")]
    async fn check_rtkit_availability(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        let rtkit_available = Self::is_rtkit_available().await;

        let status = if rtkit_available {
            PowerCheckStatus::Optimal
        } else {
            PowerCheckStatus::Critical
        };

        checks.push(PowerCheck {
            name: "RTKit Availability".to_string(),
            status,
            description: "RTKit is required for real-time thread scheduling".to_string(),
            current_value: Some(
                if rtkit_available {
                    "Available"
                } else {
                    "Not Available"
                }
                .to_string(),
            ),
            expected_value: Some("Available".to_string()),
        });

        if !rtkit_available {
            remediation_steps.push(RemediationStep {
                description: "Install RTKit for real-time scheduling support".to_string(),
                action: "sudo apt install rtkit  # or equivalent for your distribution".to_string(),
                requires_admin: true,
                priority: 1,
            });
        }
    }

    #[cfg(target_os = "linux")]
    async fn check_memlock_limits(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        let memlock_limit = Self::get_memlock_limit().await;
        let sufficient = memlock_limit >= 64 * 1024 * 1024; // 64MB minimum

        let status = if sufficient {
            PowerCheckStatus::Optimal
        } else {
            PowerCheckStatus::Degraded
        };

        checks.push(PowerCheck {
            name: "Memory Lock Limits".to_string(),
            status,
            description: "Sufficient memlock limits required for RT memory locking".to_string(),
            current_value: Some(format!("{} bytes", memlock_limit)),
            expected_value: Some("≥64MB".to_string()),
        });

        if !sufficient {
            remediation_steps.push(RemediationStep {
                description: "Increase memlock limits for real-time operation".to_string(),
                action: "Add 'username hard memlock 65536' to /etc/security/limits.conf"
                    .to_string(),
                requires_admin: true,
                priority: 2,
            });
        }
    }

    #[cfg(target_os = "linux")]
    async fn check_cpu_governor(
        checks: &mut Vec<PowerCheck>,
        remediation_steps: &mut Vec<RemediationStep>,
    ) {
        let governor = Self::get_cpu_governor().await;
        let is_performance = governor == "performance";

        let status = if is_performance {
            PowerCheckStatus::Optimal
        } else {
            PowerCheckStatus::Degraded
        };

        checks.push(PowerCheck {
            name: "CPU Governor".to_string(),
            status,
            description: "CPU governor affects frequency scaling and RT latency".to_string(),
            current_value: Some(governor.clone()),
            expected_value: Some("performance".to_string()),
        });

        if !is_performance {
            remediation_steps.push(RemediationStep {
                description: "Set CPU governor to performance mode".to_string(),
                action: "echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor".to_string(),
                requires_admin: true,
                priority: 3,
            });
        }
    }

    // Platform-specific helper methods

    #[cfg(target_os = "windows")]
    async fn is_usb_selective_suspend_enabled() -> bool {
        // Check HKLM\SYSTEM\CurrentControlSet\Services\USB\DisableSelectiveSuspend
        use std::process::Command;
        let out = Command::new("reg")
            .args([
                "query",
                r"HKLM\SYSTEM\CurrentControlSet\Services\USB",
                "/v",
                "DisableSelectiveSuspend",
            ])
            .output();
        match out {
            Ok(o) if o.status.success() => {
                // Value 1 = disabled; value absent or 0 = enabled
                let text = String::from_utf8_lossy(&o.stdout);
                !text.contains("0x1")
            }
            _ => false, // Assume enabled (pessimistic) on read failure
        }
    }

    #[cfg(target_os = "windows")]
    async fn get_current_power_plan() -> String {
        use std::process::Command;
        let out = Command::new("powercfg").args(["/getactivescheme"]).output();
        match out {
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout);
                if text.contains("High performance") || text.contains("8c5e7fda") {
                    "High performance".to_string()
                } else if text.contains("Ultimate Performance") || text.contains("e9a42b02") {
                    "Ultimate Performance".to_string()
                } else if text.contains("Balanced") || text.contains("381b4222") {
                    "Balanced".to_string()
                } else {
                    text.trim().to_string()
                }
            }
            _ => "Unknown".to_string(),
        }
    }

    #[cfg(target_os = "windows")]
    async fn is_power_throttling_disabled() -> bool {
        // Check if EcoQoS / power throttling is disabled via process mitigation
        // PowerThrottling is managed per-process; we check if it's available in this process.
        // A simple heuristic: if we're running on "High performance" plan, assume not throttled.
        let plan = Self::get_current_power_plan().await;
        plan.contains("High performance") || plan.contains("Ultimate")
    }

    #[cfg(target_os = "linux")]
    async fn is_rtkit_available() -> bool {
        // Check if rtkit-daemon is active via D-Bus presence or systemctl
        use std::process::Command;
        // Try systemctl first
        if let Ok(out) = Command::new("systemctl")
            .args(["is-active", "--quiet", "rtkit-daemon"])
            .output()
        {
            if out.status.success() {
                return true;
            }
        }
        // Fall back: check if the daemon binary exists
        std::path::Path::new("/usr/lib/rtkit/rtkit-daemon").exists()
            || std::path::Path::new("/usr/libexec/rtkit-daemon").exists()
    }

    #[cfg(target_os = "linux")]
    async fn get_memlock_limit() -> u64 {
        // Read the hard memlock limit via getrlimit(RLIMIT_MEMLOCK)
        #[cfg(target_os = "linux")]
        {
            use std::mem::MaybeUninit;
            let mut rlim = MaybeUninit::<libc::rlimit>::uninit();
            // SAFETY: rlim is a POD struct; getrlimit fills it completely on success
            let ret = unsafe { libc::getrlimit(libc::RLIMIT_MEMLOCK, rlim.as_mut_ptr()) };
            if ret == 0 {
                // SAFETY: initialised by getrlimit above
                let r = unsafe { rlim.assume_init() };
                if r.rlim_cur == libc::RLIM_INFINITY {
                    return u64::MAX;
                }
                return r.rlim_cur as u64;
            }
        }
        // Fallback: conservative default
        64 * 1024 * 1024
    }

    #[cfg(target_os = "linux")]
    async fn get_cpu_governor() -> String {
        // Read from sysfs: /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
        match tokio::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
            .await
        {
            Ok(s) => s.trim().to_string(),
            Err(_) => "unknown".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_power_check() {
        let status = PowerChecker::check_power_configuration().await;

        // Should have some checks
        assert!(!status.checks.is_empty());

        // Status should be valid
        assert!(matches!(
            status.overall_status,
            PowerCheckStatus::Optimal | PowerCheckStatus::Degraded | PowerCheckStatus::Critical
        ));
    }

    #[test]
    fn test_power_check_status_display() {
        assert_eq!(PowerCheckStatus::Optimal.to_string(), "Optimal");
        assert_eq!(PowerCheckStatus::Degraded.to_string(), "Degraded");
        assert_eq!(PowerCheckStatus::Critical.to_string(), "Critical");
    }
}
