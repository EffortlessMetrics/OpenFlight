// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Virtual input controller with configurable axes, buttons, and hat switches.
//!
//! [`VirtualController`] provides a software-only input device that can be
//! driven programmatically for testing or headless operation.

use crate::backend::HatDirection;

/// Configuration for a [`VirtualController`].
#[derive(Debug, Clone)]
pub struct VirtualControllerConfig {
    /// Human-readable name.
    pub name: String,
    /// Number of axes (default 8).
    pub axis_count: usize,
    /// Number of buttons (default 32).
    pub button_count: usize,
    /// Number of hat switches (default 1).
    pub hat_count: usize,
}

impl Default for VirtualControllerConfig {
    fn default() -> Self {
        Self {
            name: "Virtual Controller".to_string(),
            axis_count: 8,
            button_count: 32,
            hat_count: 1,
        }
    }
}

/// Immutable snapshot of controller state at a point in time.
#[derive(Debug, Clone)]
pub struct ControllerSnapshot {
    /// Axis values (each in `[-1.0, 1.0]`).
    pub axes: Vec<f64>,
    /// Button pressed states.
    pub buttons: Vec<bool>,
    /// Hat switch directions.
    pub hats: Vec<HatDirection>,
}

/// A virtual input controller with configurable axes, buttons, and hats.
pub struct VirtualController {
    config: VirtualControllerConfig,
    axes: Vec<f64>,
    buttons: Vec<bool>,
    hats: Vec<HatDirection>,
}

impl VirtualController {
    /// Create a new controller from the given configuration.
    pub fn new(config: VirtualControllerConfig) -> Self {
        let axes = vec![0.0; config.axis_count];
        let buttons = vec![false; config.button_count];
        let hats = vec![HatDirection::Centered; config.hat_count];
        Self {
            config,
            axes,
            buttons,
            hats,
        }
    }

    /// Borrow the configuration.
    pub fn config(&self) -> &VirtualControllerConfig {
        &self.config
    }

    /// Set an axis value, clamped to `[-1.0, 1.0]`.
    ///
    /// Out-of-bounds indices are silently ignored.
    pub fn set_axis(&mut self, index: usize, value: f64) {
        if let Some(slot) = self.axes.get_mut(index) {
            *slot = value.clamp(-1.0, 1.0);
        }
    }

    /// Set a button's pressed state.
    ///
    /// Out-of-bounds indices are silently ignored.
    pub fn set_button(&mut self, index: usize, pressed: bool) {
        if let Some(slot) = self.buttons.get_mut(index) {
            *slot = pressed;
        }
    }

    /// Set a hat switch direction.
    ///
    /// Out-of-bounds indices are silently ignored.
    pub fn set_hat(&mut self, index: usize, direction: HatDirection) {
        if let Some(slot) = self.hats.get_mut(index) {
            *slot = direction;
        }
    }

    /// Return the current axis value, or `None` for out-of-bounds.
    pub fn get_axis(&self, index: usize) -> Option<f64> {
        self.axes.get(index).copied()
    }

    /// Return the current button state, or `None` for out-of-bounds.
    pub fn get_button(&self, index: usize) -> Option<bool> {
        self.buttons.get(index).copied()
    }

    /// Return the current hat direction, or `None` for out-of-bounds.
    pub fn get_hat(&self, index: usize) -> Option<HatDirection> {
        self.hats.get(index).copied()
    }

    /// Take an immutable snapshot of the entire controller state.
    pub fn snapshot(&self) -> ControllerSnapshot {
        ControllerSnapshot {
            axes: self.axes.clone(),
            buttons: self.buttons.clone(),
            hats: self.hats.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = VirtualControllerConfig::default();
        assert_eq!(cfg.axis_count, 8);
        assert_eq!(cfg.button_count, 32);
        assert_eq!(cfg.hat_count, 1);
    }

    #[test]
    fn test_initial_state() {
        let ctrl = VirtualController::new(VirtualControllerConfig::default());
        let snap = ctrl.snapshot();
        assert_eq!(snap.axes.len(), 8);
        assert!(snap.axes.iter().all(|&v| v == 0.0));
        assert_eq!(snap.buttons.len(), 32);
        assert!(snap.buttons.iter().all(|&b| !b));
        assert_eq!(snap.hats.len(), 1);
        assert_eq!(snap.hats[0], HatDirection::Centered);
    }

    #[test]
    fn test_axis_set_and_get() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig::default());
        ctrl.set_axis(0, 0.5);
        ctrl.set_axis(1, -0.75);
        assert!((ctrl.get_axis(0).unwrap() - 0.5).abs() < f64::EPSILON);
        assert!((ctrl.get_axis(1).unwrap() - (-0.75)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_axis_clamping_positive() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig::default());
        ctrl.set_axis(0, 5.0);
        assert!((ctrl.get_axis(0).unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_axis_clamping_negative() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig::default());
        ctrl.set_axis(0, -10.0);
        assert!((ctrl.get_axis(0).unwrap() - (-1.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_button_toggle() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig::default());
        assert!(!ctrl.get_button(5).unwrap());
        ctrl.set_button(5, true);
        assert!(ctrl.get_button(5).unwrap());
        ctrl.set_button(5, false);
        assert!(!ctrl.get_button(5).unwrap());
    }

    #[test]
    fn test_hat_directions() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig {
            hat_count: 2,
            ..Default::default()
        });
        ctrl.set_hat(0, HatDirection::North);
        ctrl.set_hat(1, HatDirection::SouthWest);
        assert_eq!(ctrl.get_hat(0).unwrap(), HatDirection::North);
        assert_eq!(ctrl.get_hat(1).unwrap(), HatDirection::SouthWest);
    }

    #[test]
    fn test_all_hat_directions() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig {
            hat_count: 9,
            ..Default::default()
        });
        let dirs = [
            HatDirection::Centered,
            HatDirection::North,
            HatDirection::NorthEast,
            HatDirection::East,
            HatDirection::SouthEast,
            HatDirection::South,
            HatDirection::SouthWest,
            HatDirection::West,
            HatDirection::NorthWest,
        ];
        for (i, &d) in dirs.iter().enumerate() {
            ctrl.set_hat(i, d);
        }
        for (i, &d) in dirs.iter().enumerate() {
            assert_eq!(ctrl.get_hat(i).unwrap(), d);
        }
    }

    #[test]
    fn test_snapshot_consistency() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig::default());
        ctrl.set_axis(0, 0.3);
        ctrl.set_button(2, true);
        ctrl.set_hat(0, HatDirection::East);

        let snap = ctrl.snapshot();
        assert!((snap.axes[0] - 0.3).abs() < f64::EPSILON);
        assert!(snap.buttons[2]);
        assert_eq!(snap.hats[0], HatDirection::East);

        // Mutating after snapshot doesn't change it.
        ctrl.set_axis(0, -0.9);
        assert!((snap.axes[0] - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_oob_axis_ignored() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig {
            axis_count: 2,
            ..Default::default()
        });
        ctrl.set_axis(99, 0.5);
        assert!(ctrl.get_axis(99).is_none());
    }

    #[test]
    fn test_oob_button_ignored() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig {
            button_count: 4,
            ..Default::default()
        });
        ctrl.set_button(100, true);
        assert!(ctrl.get_button(100).is_none());
    }

    #[test]
    fn test_oob_hat_ignored() {
        let mut ctrl = VirtualController::new(VirtualControllerConfig {
            hat_count: 1,
            ..Default::default()
        });
        ctrl.set_hat(5, HatDirection::North);
        assert!(ctrl.get_hat(5).is_none());
    }

    #[test]
    fn test_custom_config() {
        let cfg = VirtualControllerConfig {
            name: "Throttle".to_string(),
            axis_count: 4,
            button_count: 16,
            hat_count: 0,
        };
        let ctrl = VirtualController::new(cfg);
        assert_eq!(ctrl.config().name, "Throttle");
        assert_eq!(ctrl.snapshot().axes.len(), 4);
        assert_eq!(ctrl.snapshot().buttons.len(), 16);
        assert_eq!(ctrl.snapshot().hats.len(), 0);
    }
}
