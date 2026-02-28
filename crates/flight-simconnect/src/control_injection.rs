// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Control injection for writing processed values back to MSFS via SimConnect.
//!
//! `SimControlInjector` wraps axis-set events, key events, and simulation
//! variable writes behind a rate-limited façade that prevents flooding the
//! SimConnect dispatch queue. Each call is tracked so metrics can be reported
//! through the standard adapter counters.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Axis ID → SimConnect event mapping
// ---------------------------------------------------------------------------

/// Well-known axis identifiers used by the Flight Hub axis engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisId {
    Elevator,
    Ailerons,
    Rudder,
    Throttle,
    Throttle1,
    Throttle2,
    Mixture,
    Propeller,
    Spoiler,
    Flaps,
    BrakeLeft,
    BrakeRight,
}

impl AxisId {
    /// Return the SimConnect `AXIS_*_SET` event name for this axis.
    pub fn simconnect_event(self) -> &'static str {
        match self {
            Self::Elevator => "AXIS_ELEVATOR_SET",
            Self::Ailerons => "AXIS_AILERONS_SET",
            Self::Rudder => "AXIS_RUDDER_SET",
            Self::Throttle => "AXIS_THROTTLE_SET",
            Self::Throttle1 => "AXIS_THROTTLE1_SET",
            Self::Throttle2 => "AXIS_THROTTLE2_SET",
            Self::Mixture => "AXIS_MIXTURE_SET",
            Self::Propeller => "AXIS_PROPELLER_SET",
            Self::Spoiler => "AXIS_SPOILER_SET",
            Self::Flaps => "AXIS_FLAPS_SET",
            Self::BrakeLeft => "AXIS_LEFT_BRAKE_SET",
            Self::BrakeRight => "AXIS_RIGHT_BRAKE_SET",
        }
    }

    /// Try to parse a string axis name (case-insensitive) into an `AxisId`.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "ELEVATOR" => Some(Self::Elevator),
            "AILERONS" | "AILERON" => Some(Self::Ailerons),
            "RUDDER" => Some(Self::Rudder),
            "THROTTLE" => Some(Self::Throttle),
            "THROTTLE1" => Some(Self::Throttle1),
            "THROTTLE2" => Some(Self::Throttle2),
            "MIXTURE" => Some(Self::Mixture),
            "PROPELLER" | "PROP" => Some(Self::Propeller),
            "SPOILER" | "SPEEDBRAKE" => Some(Self::Spoiler),
            "FLAPS" => Some(Self::Flaps),
            "BRAKE_LEFT" | "BRAKELEFT" => Some(Self::BrakeLeft),
            "BRAKE_RIGHT" | "BRAKERIGHT" => Some(Self::BrakeRight),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Injection command
// ---------------------------------------------------------------------------

/// A single control-injection command ready to be dispatched.
#[derive(Debug, Clone, PartialEq)]
pub enum InjectionCommand {
    /// Set an axis via an AXIS_*_SET event. Value is in the SimConnect
    /// signed-16-bit range (−16384 … +16383).
    SetAxis { axis: AxisId, value: i32 },
    /// Fire a client event by name (K: key event). Optional `data` parameter.
    TriggerEvent { event_name: String, data: u32 },
    /// Write a named simulation variable. Uses `SetDataOnSimObject`.
    SetVariable { var_name: String, value: f64 },
}

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// Token-bucket rate limiter — allows `capacity` commands per `window`.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    capacity: u32,
    window: Duration,
    tokens: u32,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a limiter that allows `capacity` commands per `window`.
    pub fn new(capacity: u32, window: Duration) -> Self {
        Self {
            capacity,
            window,
            tokens: capacity,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume one token. Returns `true` if the command is allowed.
    pub fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Available tokens without consuming.
    pub fn available(&mut self) -> u32 {
        self.refill();
        self.tokens
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        if elapsed >= self.window {
            self.tokens = self.capacity;
            self.last_refill = now;
        }
    }
}

// ---------------------------------------------------------------------------
// SimControlInjector
// ---------------------------------------------------------------------------

/// Configuration for the control injector.
#[derive(Debug, Clone)]
pub struct ControlInjectorConfig {
    /// Whether injection is enabled (false by default for safety).
    pub enabled: bool,
    /// Maximum commands per second before rate-limiting kicks in.
    pub max_commands_per_sec: u32,
}

impl Default for ControlInjectorConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_commands_per_sec: 200,
        }
    }
}

/// Writes control inputs back to MSFS via SimConnect.
///
/// Tracks every command in an outbound queue so callers can inspect pending
/// work and replay on reconnection. Atomic counters let any thread read
/// statistics without taking a lock.
pub struct SimControlInjector {
    config: ControlInjectorConfig,
    rate_limiter: RateLimiter,
    /// Pre-resolved axis-event names for quick lookup.
    axis_events: HashMap<AxisId, &'static str>,
    /// Outbound command queue (drained by the send loop).
    pending: Vec<InjectionCommand>,
    /// Lifetime counters.
    commands_sent: AtomicU64,
    commands_dropped: AtomicU64,
    errors: AtomicU64,
}

impl SimControlInjector {
    /// Create a new injector with the given configuration.
    pub fn new(config: ControlInjectorConfig) -> Self {
        let rate_limiter = RateLimiter::new(config.max_commands_per_sec, Duration::from_secs(1));

        // Pre-populate axis event map for the well-known axes.
        let axis_events: HashMap<AxisId, &'static str> = [
            AxisId::Elevator,
            AxisId::Ailerons,
            AxisId::Rudder,
            AxisId::Throttle,
            AxisId::Throttle1,
            AxisId::Throttle2,
            AxisId::Mixture,
            AxisId::Propeller,
            AxisId::Spoiler,
            AxisId::Flaps,
            AxisId::BrakeLeft,
            AxisId::BrakeRight,
        ]
        .iter()
        .map(|a| (*a, a.simconnect_event()))
        .collect();

        Self {
            config,
            rate_limiter,
            axis_events,
            pending: Vec::new(),
            commands_sent: AtomicU64::new(0),
            commands_dropped: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }

    /// Returns `true` if the injector is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    // -- command builders --

    /// Queue an axis-set command. `value` is clamped to −16384 … +16383.
    pub fn set_axis(&mut self, axis: AxisId, value: i32) -> bool {
        if !self.config.enabled {
            return false;
        }
        let clamped = value.clamp(-16384, 16383);
        self.enqueue(InjectionCommand::SetAxis {
            axis,
            value: clamped,
        })
    }

    /// Queue a client-event trigger (K: key event).
    pub fn trigger_event(&mut self, event_name: &str, data: u32) -> bool {
        if !self.config.enabled {
            return false;
        }
        self.enqueue(InjectionCommand::TriggerEvent {
            event_name: event_name.to_string(),
            data,
        })
    }

    /// Queue a simulation-variable write.
    pub fn set_variable(&mut self, var_name: &str, value: f64) -> bool {
        if !self.config.enabled {
            return false;
        }
        self.enqueue(InjectionCommand::SetVariable {
            var_name: var_name.to_string(),
            value,
        })
    }

    // -- queue access --

    /// Take all pending commands out of the queue, leaving it empty.
    pub fn drain_pending(&mut self) -> Vec<InjectionCommand> {
        std::mem::take(&mut self.pending)
    }

    /// Number of commands waiting to be sent.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    // -- counters --

    pub fn commands_sent(&self) -> u64 {
        self.commands_sent.load(Ordering::Relaxed)
    }

    pub fn commands_dropped(&self) -> u64 {
        self.commands_dropped.load(Ordering::Relaxed)
    }

    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    /// Record that `n` commands were successfully dispatched.
    pub fn record_sent(&self, n: u64) {
        self.commands_sent.fetch_add(n, Ordering::Relaxed);
    }

    /// Record a dispatch error.
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Look up the SimConnect event name for a given axis.
    pub fn axis_event_name(&self, axis: AxisId) -> Option<&&'static str> {
        self.axis_events.get(&axis)
    }

    // -- internal helpers --

    fn enqueue(&mut self, cmd: InjectionCommand) -> bool {
        if self.rate_limiter.try_acquire() {
            self.pending.push(cmd);
            true
        } else {
            self.commands_dropped.fetch_add(1, Ordering::Relaxed);
            false
        }
    }
}

impl Default for SimControlInjector {
    fn default() -> Self {
        Self::new(ControlInjectorConfig::default())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- AxisId mapping --

    #[test]
    fn axis_id_to_simconnect_event() {
        assert_eq!(AxisId::Elevator.simconnect_event(), "AXIS_ELEVATOR_SET");
        assert_eq!(AxisId::Ailerons.simconnect_event(), "AXIS_AILERONS_SET");
        assert_eq!(AxisId::Rudder.simconnect_event(), "AXIS_RUDDER_SET");
        assert_eq!(AxisId::Throttle.simconnect_event(), "AXIS_THROTTLE_SET");
        assert_eq!(AxisId::Throttle1.simconnect_event(), "AXIS_THROTTLE1_SET");
        assert_eq!(AxisId::Throttle2.simconnect_event(), "AXIS_THROTTLE2_SET");
        assert_eq!(AxisId::Mixture.simconnect_event(), "AXIS_MIXTURE_SET");
        assert_eq!(AxisId::Propeller.simconnect_event(), "AXIS_PROPELLER_SET");
        assert_eq!(AxisId::Spoiler.simconnect_event(), "AXIS_SPOILER_SET");
        assert_eq!(AxisId::Flaps.simconnect_event(), "AXIS_FLAPS_SET");
        assert_eq!(AxisId::BrakeLeft.simconnect_event(), "AXIS_LEFT_BRAKE_SET");
        assert_eq!(
            AxisId::BrakeRight.simconnect_event(),
            "AXIS_RIGHT_BRAKE_SET"
        );
    }

    #[test]
    fn axis_id_from_name_known() {
        assert_eq!(AxisId::from_name("elevator"), Some(AxisId::Elevator));
        assert_eq!(AxisId::from_name("AILERONS"), Some(AxisId::Ailerons));
        assert_eq!(AxisId::from_name("aileron"), Some(AxisId::Ailerons));
        assert_eq!(AxisId::from_name("Rudder"), Some(AxisId::Rudder));
        assert_eq!(AxisId::from_name("THROTTLE"), Some(AxisId::Throttle));
        assert_eq!(AxisId::from_name("throttle1"), Some(AxisId::Throttle1));
        assert_eq!(AxisId::from_name("MIXTURE"), Some(AxisId::Mixture));
        assert_eq!(AxisId::from_name("prop"), Some(AxisId::Propeller));
        assert_eq!(AxisId::from_name("PROPELLER"), Some(AxisId::Propeller));
        assert_eq!(AxisId::from_name("speedbrake"), Some(AxisId::Spoiler));
        assert_eq!(AxisId::from_name("flaps"), Some(AxisId::Flaps));
        assert_eq!(AxisId::from_name("BRAKE_LEFT"), Some(AxisId::BrakeLeft));
        assert_eq!(AxisId::from_name("brakeright"), Some(AxisId::BrakeRight));
    }

    #[test]
    fn axis_id_from_name_unknown() {
        assert_eq!(AxisId::from_name("nosegear"), None);
        assert_eq!(AxisId::from_name(""), None);
    }

    // -- InjectionCommand --

    #[test]
    fn injection_command_set_axis() {
        let cmd = InjectionCommand::SetAxis {
            axis: AxisId::Elevator,
            value: 8192,
        };
        assert_eq!(
            cmd,
            InjectionCommand::SetAxis {
                axis: AxisId::Elevator,
                value: 8192,
            }
        );
    }

    #[test]
    fn injection_command_trigger_event() {
        let cmd = InjectionCommand::TriggerEvent {
            event_name: "GEAR_TOGGLE".to_string(),
            data: 0,
        };
        if let InjectionCommand::TriggerEvent { event_name, data } = &cmd {
            assert_eq!(event_name, "GEAR_TOGGLE");
            assert_eq!(*data, 0);
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn injection_command_set_variable() {
        let cmd = InjectionCommand::SetVariable {
            var_name: "GENERAL ENG THROTTLE LEVER POSITION:1".to_string(),
            value: 75.0,
        };
        if let InjectionCommand::SetVariable { var_name, value } = &cmd {
            assert_eq!(var_name, "GENERAL ENG THROTTLE LEVER POSITION:1");
            assert!((value - 75.0).abs() < f64::EPSILON);
        } else {
            panic!("unexpected variant");
        }
    }

    // -- RateLimiter --

    #[test]
    fn rate_limiter_allows_up_to_capacity() {
        let mut rl = RateLimiter::new(5, Duration::from_secs(1));
        for _ in 0..5 {
            assert!(rl.try_acquire());
        }
        assert!(!rl.try_acquire(), "should be exhausted");
    }

    #[test]
    fn rate_limiter_available() {
        let mut rl = RateLimiter::new(10, Duration::from_secs(1));
        assert_eq!(rl.available(), 10);
        rl.try_acquire();
        rl.try_acquire();
        assert_eq!(rl.available(), 8);
    }

    #[test]
    fn rate_limiter_refills_after_window() {
        let mut rl = RateLimiter::new(3, Duration::from_millis(1));
        for _ in 0..3 {
            rl.try_acquire();
        }
        assert!(!rl.try_acquire());
        // Sleep past the window to trigger refill.
        std::thread::sleep(Duration::from_millis(5));
        assert!(rl.try_acquire(), "should have refilled");
    }

    // -- ControlInjectorConfig --

    #[test]
    fn config_defaults() {
        let cfg = ControlInjectorConfig::default();
        assert!(!cfg.enabled, "injection must be disabled by default");
        assert_eq!(cfg.max_commands_per_sec, 200);
    }

    // -- SimControlInjector --

    fn enabled_injector() -> SimControlInjector {
        SimControlInjector::new(ControlInjectorConfig {
            enabled: true,
            max_commands_per_sec: 100,
        })
    }

    #[test]
    fn injector_disabled_by_default() {
        let mut inj = SimControlInjector::default();
        assert!(!inj.is_enabled());
        assert!(!inj.set_axis(AxisId::Elevator, 0));
        assert!(!inj.trigger_event("GEAR_TOGGLE", 0));
        assert!(!inj.set_variable("AILERON POSITION", 0.5));
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn set_axis_queues_command() {
        let mut inj = enabled_injector();
        assert!(inj.set_axis(AxisId::Elevator, 4096));
        assert_eq!(inj.pending_count(), 1);
        let cmds = inj.drain_pending();
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            cmds[0],
            InjectionCommand::SetAxis {
                axis: AxisId::Elevator,
                value: 4096,
            }
        );
    }

    #[test]
    fn set_axis_clamps_value() {
        let mut inj = enabled_injector();
        inj.set_axis(AxisId::Rudder, 99999);
        inj.set_axis(AxisId::Ailerons, -99999);
        let cmds = inj.drain_pending();
        if let InjectionCommand::SetAxis { value, .. } = cmds[0] {
            assert_eq!(value, 16383);
        }
        if let InjectionCommand::SetAxis { value, .. } = cmds[1] {
            assert_eq!(value, -16384);
        }
    }

    #[test]
    fn trigger_event_queues_command() {
        let mut inj = enabled_injector();
        assert!(inj.trigger_event("AP_MASTER", 0));
        let cmds = inj.drain_pending();
        assert_eq!(
            cmds[0],
            InjectionCommand::TriggerEvent {
                event_name: "AP_MASTER".to_string(),
                data: 0,
            }
        );
    }

    #[test]
    fn set_variable_queues_command() {
        let mut inj = enabled_injector();
        assert!(inj.set_variable("AILERON POSITION", 0.25));
        let cmds = inj.drain_pending();
        assert_eq!(
            cmds[0],
            InjectionCommand::SetVariable {
                var_name: "AILERON POSITION".to_string(),
                value: 0.25,
            }
        );
    }

    #[test]
    fn drain_clears_queue() {
        let mut inj = enabled_injector();
        inj.set_axis(AxisId::Throttle, 8000);
        inj.trigger_event("GEAR_TOGGLE", 0);
        assert_eq!(inj.pending_count(), 2);
        let _ = inj.drain_pending();
        assert_eq!(inj.pending_count(), 0);
    }

    #[test]
    fn rate_limiting_drops_commands() {
        let mut inj = SimControlInjector::new(ControlInjectorConfig {
            enabled: true,
            max_commands_per_sec: 3,
        });

        // First 3 should succeed
        assert!(inj.set_axis(AxisId::Elevator, 0));
        assert!(inj.set_axis(AxisId::Ailerons, 0));
        assert!(inj.set_axis(AxisId::Rudder, 0));

        // 4th should be dropped
        assert!(!inj.set_axis(AxisId::Throttle, 0));
        assert_eq!(inj.pending_count(), 3);
        assert_eq!(inj.commands_dropped(), 1);
    }

    #[test]
    fn counters_start_at_zero() {
        let inj = SimControlInjector::default();
        assert_eq!(inj.commands_sent(), 0);
        assert_eq!(inj.commands_dropped(), 0);
        assert_eq!(inj.errors(), 0);
    }

    #[test]
    fn record_sent_increments() {
        let inj = enabled_injector();
        inj.record_sent(5);
        assert_eq!(inj.commands_sent(), 5);
        inj.record_sent(3);
        assert_eq!(inj.commands_sent(), 8);
    }

    #[test]
    fn record_error_increments() {
        let inj = enabled_injector();
        inj.record_error();
        inj.record_error();
        assert_eq!(inj.errors(), 2);
    }

    #[test]
    fn axis_event_name_lookup() {
        let inj = enabled_injector();
        assert_eq!(
            inj.axis_event_name(AxisId::Elevator),
            Some(&"AXIS_ELEVATOR_SET")
        );
        assert_eq!(inj.axis_event_name(AxisId::Flaps), Some(&"AXIS_FLAPS_SET"));
    }

    #[test]
    fn multiple_command_types_interleave() {
        let mut inj = enabled_injector();
        inj.set_axis(AxisId::Elevator, 100);
        inj.trigger_event("GEAR_TOGGLE", 0);
        inj.set_variable("AILERON POSITION", 0.5);
        inj.set_axis(AxisId::Rudder, -200);

        let cmds = inj.drain_pending();
        assert_eq!(cmds.len(), 4);
        assert!(matches!(cmds[0], InjectionCommand::SetAxis { .. }));
        assert!(matches!(cmds[1], InjectionCommand::TriggerEvent { .. }));
        assert!(matches!(cmds[2], InjectionCommand::SetVariable { .. }));
        assert!(matches!(cmds[3], InjectionCommand::SetAxis { .. }));
    }
}
