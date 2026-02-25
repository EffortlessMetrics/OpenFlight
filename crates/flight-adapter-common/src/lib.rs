// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Common adapter types shared across simulator adapters.

pub mod config;
pub mod error;
pub mod metrics;
pub mod reconnection;
pub mod state;

pub use config::AdapterConfig;
pub use error::AdapterError;
pub use metrics::AdapterMetrics;
pub use reconnection::ReconnectionStrategy;
pub use state::AdapterState;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_error_display_variants() {
        assert_eq!(AdapterError::NotConnected.to_string(), "Not connected");
        let t = AdapterError::Timeout("deadline exceeded".to_string());
        assert!(t.to_string().contains("deadline exceeded"));
        assert_eq!(
            AdapterError::AircraftNotDetected.to_string(),
            "Aircraft not detected"
        );
        assert!(
            AdapterError::Configuration("bad key".to_string())
                .to_string()
                .contains("bad key")
        );
        assert_eq!(
            AdapterError::ReconnectExhausted.to_string(),
            "Reconnect attempts exhausted"
        );
        let other = AdapterError::Other("custom".to_string());
        assert!(other.to_string().contains("custom"));
    }

    #[test]
    fn adapter_state_equality() {
        assert_eq!(AdapterState::Connected, AdapterState::Connected);
        assert_ne!(AdapterState::Connected, AdapterState::Active);
        assert_ne!(AdapterState::Disconnected, AdapterState::Error);
    }

    #[test]
    fn adapter_metrics_summary_format() {
        let mut m = AdapterMetrics::new();
        m.record_update();
        m.record_aircraft_change("C172".to_string());
        let s = m.summary();
        assert!(s.contains("Updates:"), "got: {s}");
        assert!(s.contains("Aircraft changes:"), "got: {s}");
    }

    #[test]
    fn reconnection_strategy_max_backoff_caps() {
        use std::time::Duration;
        let s = ReconnectionStrategy::new(
            10,
            Duration::from_millis(100),
            Duration::from_millis(500),
        );
        // Very high attempt should hit the max cap
        assert_eq!(s.next_backoff(20), Duration::from_millis(500));
    }
}
