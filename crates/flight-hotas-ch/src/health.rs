// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Health and connectivity status types for CH Products devices.

use flight_hid_support::device_support::ChModel;

/// Connectivity status of a CH Products device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChHealthStatus {
    /// Device is present and delivering HID reports.
    Connected,
    /// Device is not detected on any HID path.
    Disconnected,
    /// Status has not yet been determined (initial state).
    Unknown,
}

/// Health monitor for a single CH Products device.
#[derive(Debug)]
pub struct ChHealthMonitor {
    model: ChModel,
    status: ChHealthStatus,
}

impl ChHealthMonitor {
    /// Create a new monitor for the given device model.
    ///
    /// Initial status is [`ChHealthStatus::Unknown`].
    pub fn new(model: ChModel) -> Self {
        Self {
            model,
            status: ChHealthStatus::Unknown,
        }
    }

    /// Update the connectivity status.
    pub fn update_status(&mut self, status: ChHealthStatus) {
        self.status = status;
    }

    /// Return the current connectivity status.
    pub fn status(&self) -> &ChHealthStatus {
        &self.status
    }

    /// Return the device model this monitor tracks.
    pub fn model(&self) -> ChModel {
        self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_status_is_unknown() {
        let monitor = ChHealthMonitor::new(ChModel::Fighterstick);
        assert_eq!(monitor.status(), &ChHealthStatus::Unknown);
    }

    #[test]
    fn update_to_connected() {
        let mut monitor = ChHealthMonitor::new(ChModel::ProThrottle);
        monitor.update_status(ChHealthStatus::Connected);
        assert_eq!(monitor.status(), &ChHealthStatus::Connected);
    }

    #[test]
    fn update_to_disconnected() {
        let mut monitor = ChHealthMonitor::new(ChModel::ProPedals);
        monitor.update_status(ChHealthStatus::Connected);
        monitor.update_status(ChHealthStatus::Disconnected);
        assert_eq!(monitor.status(), &ChHealthStatus::Disconnected);
    }

    #[test]
    fn model_is_preserved() {
        let monitor = ChHealthMonitor::new(ChModel::EclipseYoke);
        assert_eq!(monitor.model(), ChModel::EclipseYoke);
    }

    #[test]
    fn all_models_can_be_monitored() {
        for model in [
            ChModel::ProThrottle,
            ChModel::ProPedals,
            ChModel::Fighterstick,
            ChModel::CombatStick,
            ChModel::EclipseYoke,
            ChModel::FlightYoke,
        ] {
            let mut monitor = ChHealthMonitor::new(model);
            monitor.update_status(ChHealthStatus::Connected);
            assert_eq!(monitor.status(), &ChHealthStatus::Connected);
        }
    }
}
