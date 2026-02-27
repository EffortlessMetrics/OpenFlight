// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Shared health model for device managers.

use std::time::Instant;

/// Health state for one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceHealth {
    Healthy,
    Degraded { reason: String },
    Quarantined { since: Instant, reason: String },
    Failed { error: String },
}

impl DeviceHealth {
    /// Whether the device is still usable for read/write operations.
    pub fn is_operational(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded { .. })
    }

    /// Human-readable reason for non-healthy states.
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Healthy => None,
            Self::Degraded { reason } => Some(reason),
            Self::Quarantined { reason, .. } => Some(reason),
            Self::Failed { error } => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DeviceHealth;
    use std::time::Instant;

    #[test]
    fn test_operational_states() {
        assert!(DeviceHealth::Healthy.is_operational());
        assert!(
            DeviceHealth::Degraded {
                reason: "latency".to_string()
            }
            .is_operational()
        );
        assert!(
            !DeviceHealth::Quarantined {
                since: Instant::now(),
                reason: "fault".to_string()
            }
            .is_operational()
        );
        assert!(
            !DeviceHealth::Failed {
                error: "disconnected".to_string()
            }
            .is_operational()
        );
    }

    #[test]
    fn test_reason_healthy_is_none() {
        assert!(DeviceHealth::Healthy.reason().is_none());
    }

    #[test]
    fn test_reason_degraded() {
        let h = DeviceHealth::Degraded {
            reason: "high latency".to_string(),
        };
        assert_eq!(h.reason(), Some("high latency"));
    }

    #[test]
    fn test_reason_quarantined() {
        let h = DeviceHealth::Quarantined {
            since: Instant::now(),
            reason: "too many errors".to_string(),
        };
        assert_eq!(h.reason(), Some("too many errors"));
    }

    #[test]
    fn test_reason_failed() {
        let h = DeviceHealth::Failed {
            error: "USB disconnect".to_string(),
        };
        assert_eq!(h.reason(), Some("USB disconnect"));
    }
}
