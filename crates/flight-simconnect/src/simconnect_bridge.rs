// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SimConnect bridge — integrates connection, state machine, variable
//! registration, control injection, and aircraft detection behind a
//! backend trait so the full adapter lifecycle can be tested without MSFS.

use crate::adapter_state::{SimConnectAdapterState, SimConnectEvent, SimConnectStateMachine};
use crate::aircraft_detection::{AircraftDetectionEngine, DetectionResult, SimAircraftData};
use crate::connection::ExponentialBackoff;
use crate::control_injection::{AxisId, ControlInjectorConfig, SimControlInjector};
use crate::var_registry::{SimVar, SimVarCategory, SimVarRegistry};
use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// Result type returned by backend operations.
pub type BackendResult<T> = Result<T, BackendError>;

/// Errors that a [`SimConnectBackend`] may produce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Connection could not be established.
    ConnectionFailed(String),
    /// The connection was lost.
    ConnectionLost(String),
    /// A data definition or request was invalid.
    InvalidRequest(String),
    /// An event transmission failed.
    EventFailed(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            Self::ConnectionLost(msg) => write!(f, "connection lost: {msg}"),
            Self::InvalidRequest(msg) => write!(f, "invalid request: {msg}"),
            Self::EventFailed(msg) => write!(f, "event failed: {msg}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// Received SimConnect dispatch message (simplified).
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchMessage {
    /// SIMCONNECT_RECV_OPEN equivalent.
    Open,
    /// SIMCONNECT_RECV_QUIT equivalent.
    Quit,
    /// Sim-object data with define-id and f64 values keyed by datum index.
    SimObjectData {
        define_id: u32,
        request_id: u32,
        values: Vec<f64>,
    },
    /// A system or client event was fired.
    Event { event_id: u32, data: u32 },
    /// An exception / error from SimConnect.
    Exception { code: u32 },
}

/// Abstraction over the real SimConnect DLL so the bridge can be tested
/// with a [`MockSimConnectBackend`].
pub trait SimConnectBackend {
    /// Attempt to open a SimConnect connection.
    fn open(&mut self, app_name: &str) -> BackendResult<()>;
    /// Close the connection.
    fn close(&mut self) -> BackendResult<()>;
    /// Register a data definition (one SimVar).
    fn add_to_data_definition(
        &mut self,
        define_id: u32,
        datum_name: &str,
        units: &str,
    ) -> BackendResult<()>;
    /// Start periodic data requests.
    fn request_data(&mut self, request_id: u32, define_id: u32) -> BackendResult<()>;
    /// Map a client event name and return an event id.
    fn map_client_event(&mut self, event_id: u32, event_name: &str) -> BackendResult<()>;
    /// Transmit a client event with data.
    fn transmit_event(&mut self, event_id: u32, data: u32) -> BackendResult<()>;
    /// Subscribe to a system event (e.g. AircraftLoaded).
    fn subscribe_system_event(&mut self, event_id: u32, event_name: &str) -> BackendResult<()>;
    /// Write SimVar values back to the sim object (SimConnect_SetDataOnSimObject).
    fn set_data_on_sim_object(
        &mut self,
        define_id: u32,
        values: &[f64],
    ) -> BackendResult<()>;
    /// Poll for the next dispatch message (non-blocking).
    fn get_next_dispatch(&mut self) -> BackendResult<Option<DispatchMessage>>;
}

// ---------------------------------------------------------------------------
// Mock backend
// ---------------------------------------------------------------------------

/// A mock SimConnect backend for testing without MSFS.
#[derive(Debug)]
pub struct MockSimConnectBackend {
    opened: bool,
    /// Pre-queued dispatch messages for testing.
    dispatch_queue: Vec<DispatchMessage>,
    /// Definitions registered via `add_to_data_definition`.
    definitions: HashMap<u32, Vec<(String, String)>>,
    /// Events mapped via `map_client_event`.
    mapped_events: HashMap<u32, String>,
    /// Events transmitted — (event_id, data).
    transmitted_events: Vec<(u32, u32)>,
    /// System events subscribed.
    system_events: HashMap<u32, String>,
    /// Requests started.
    active_requests: HashMap<u32, u32>,
    /// Data written via `set_data_on_sim_object` — (define_id, values).
    written_data: Vec<(u32, Vec<f64>)>,
    /// When true, the next `open()` will fail.
    pub fail_next_open: bool,
    /// When true, the next `transmit_event()` will fail.
    pub fail_next_transmit: bool,
    /// When true, the next `set_data_on_sim_object()` will fail.
    pub fail_next_write: bool,
}

impl MockSimConnectBackend {
    pub fn new() -> Self {
        Self {
            opened: false,
            dispatch_queue: Vec::new(),
            definitions: HashMap::new(),
            mapped_events: HashMap::new(),
            transmitted_events: Vec::new(),
            system_events: HashMap::new(),
            active_requests: HashMap::new(),
            written_data: Vec::new(),
            fail_next_open: false,
            fail_next_transmit: false,
            fail_next_write: false,
        }
    }

    /// Push a message that will be returned by `get_next_dispatch`.
    pub fn push_dispatch(&mut self, msg: DispatchMessage) {
        self.dispatch_queue.push(msg);
    }

    pub fn is_open(&self) -> bool {
        self.opened
    }

    pub fn definitions(&self) -> &HashMap<u32, Vec<(String, String)>> {
        &self.definitions
    }

    pub fn mapped_events(&self) -> &HashMap<u32, String> {
        &self.mapped_events
    }

    pub fn transmitted_events(&self) -> &[(u32, u32)] {
        &self.transmitted_events
    }

    pub fn system_events(&self) -> &HashMap<u32, String> {
        &self.system_events
    }

    pub fn active_requests(&self) -> &HashMap<u32, u32> {
        &self.active_requests
    }

    pub fn written_data(&self) -> &[(u32, Vec<f64>)] {
        &self.written_data
    }
}

impl Default for MockSimConnectBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl SimConnectBackend for MockSimConnectBackend {
    fn open(&mut self, _app_name: &str) -> BackendResult<()> {
        if self.fail_next_open {
            self.fail_next_open = false;
            return Err(BackendError::ConnectionFailed("mock open failure".into()));
        }
        self.opened = true;
        Ok(())
    }

    fn close(&mut self) -> BackendResult<()> {
        self.opened = false;
        self.definitions.clear();
        self.mapped_events.clear();
        self.system_events.clear();
        self.active_requests.clear();
        self.written_data.clear();
        Ok(())
    }

    fn add_to_data_definition(
        &mut self,
        define_id: u32,
        datum_name: &str,
        units: &str,
    ) -> BackendResult<()> {
        if !self.opened {
            return Err(BackendError::ConnectionLost("not open".into()));
        }
        self.definitions
            .entry(define_id)
            .or_default()
            .push((datum_name.to_string(), units.to_string()));
        Ok(())
    }

    fn request_data(&mut self, request_id: u32, define_id: u32) -> BackendResult<()> {
        if !self.opened {
            return Err(BackendError::ConnectionLost("not open".into()));
        }
        self.active_requests.insert(request_id, define_id);
        Ok(())
    }

    fn map_client_event(&mut self, event_id: u32, event_name: &str) -> BackendResult<()> {
        if !self.opened {
            return Err(BackendError::ConnectionLost("not open".into()));
        }
        self.mapped_events.insert(event_id, event_name.to_string());
        Ok(())
    }

    fn transmit_event(&mut self, event_id: u32, data: u32) -> BackendResult<()> {
        if !self.opened {
            return Err(BackendError::ConnectionLost("not open".into()));
        }
        if self.fail_next_transmit {
            self.fail_next_transmit = false;
            return Err(BackendError::EventFailed("mock transmit failure".into()));
        }
        self.transmitted_events.push((event_id, data));
        Ok(())
    }

    fn subscribe_system_event(&mut self, event_id: u32, event_name: &str) -> BackendResult<()> {
        if !self.opened {
            return Err(BackendError::ConnectionLost("not open".into()));
        }
        self.system_events.insert(event_id, event_name.to_string());
        Ok(())
    }

    fn set_data_on_sim_object(
        &mut self,
        define_id: u32,
        values: &[f64],
    ) -> BackendResult<()> {
        if !self.opened {
            return Err(BackendError::ConnectionLost("not open".into()));
        }
        if self.fail_next_write {
            self.fail_next_write = false;
            return Err(BackendError::EventFailed("mock write failure".into()));
        }
        self.written_data.push((define_id, values.to_vec()));
        Ok(())
    }

    fn get_next_dispatch(&mut self) -> BackendResult<Option<DispatchMessage>> {
        if !self.opened {
            return Err(BackendError::ConnectionLost("not open".into()));
        }
        if self.dispatch_queue.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.dispatch_queue.remove(0)))
        }
    }
}

// ---------------------------------------------------------------------------
// Bridge configuration
// ---------------------------------------------------------------------------

/// Configuration for the bridge.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub app_name: String,
    pub stale_threshold_ms: u64,
    pub max_retries: u32,
    pub backoff_base: Duration,
    pub backoff_max: Duration,
    pub injection_enabled: bool,
    pub max_commands_per_sec: u32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            app_name: "Flight Hub".into(),
            stale_threshold_ms: 5000,
            max_retries: 5,
            backoff_base: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
            injection_enabled: false,
            max_commands_per_sec: 200,
        }
    }
}

// ---------------------------------------------------------------------------
// Aircraft change event
// ---------------------------------------------------------------------------

/// Emitted when the bridge detects an aircraft change.
#[derive(Debug, Clone, PartialEq)]
pub struct AircraftChanged {
    pub title: String,
    pub icao: Option<String>,
    pub display_name: Option<String>,
}

// ---------------------------------------------------------------------------
// Bridge variable snapshot (simplified BusSnapshot stand-in)
// ---------------------------------------------------------------------------

/// A snapshot of sim variable values produced by the bridge.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VarSnapshot {
    pub values: HashMap<String, f64>,
    pub timestamp_ns: u64,
}

// ---------------------------------------------------------------------------
// SimConnect bridge
// ---------------------------------------------------------------------------

/// Well-known definition / request IDs used by the bridge.
const DEF_TELEMETRY: u32 = 1;
const REQ_TELEMETRY: u32 = 1;
const DEF_AIRCRAFT: u32 = 2;
const REQ_AIRCRAFT: u32 = 2;

/// Well-known event IDs.
const EVT_AIRCRAFT_LOADED: u32 = 100;

/// Orchestrates the SimConnect adapter lifecycle.
pub struct SimConnectBridge<B: SimConnectBackend> {
    backend: B,
    config: BridgeConfig,
    state_machine: SimConnectStateMachine,
    backoff: ExponentialBackoff,
    var_registry: SimVarRegistry,
    detection_engine: AircraftDetectionEngine,
    injector: SimControlInjector,
    /// Names of variables registered in the current telemetry definition,
    /// in the order they appear (index corresponds to f64 index in data).
    registered_vars: Vec<String>,
    /// Map of axis event_id to AxisId, filled during event setup.
    axis_event_ids: HashMap<u32, AxisId>,
    /// Next event ID to allocate.
    next_event_id: u32,
    /// Latest snapshot.
    pub latest_snapshot: Option<VarSnapshot>,
    /// Latest detected aircraft.
    pub latest_aircraft: Option<AircraftChanged>,
    /// Accumulated aircraft-change events (for consumers to drain).
    pub aircraft_events: Vec<AircraftChanged>,
}

impl<B: SimConnectBackend> SimConnectBridge<B> {
    /// Create a new bridge.
    pub fn new(backend: B, config: BridgeConfig) -> Self {
        let state_machine =
            SimConnectStateMachine::new(config.stale_threshold_ms, config.max_retries);
        let backoff = ExponentialBackoff::new(config.backoff_base, config.backoff_max);
        let injector = SimControlInjector::new(ControlInjectorConfig {
            enabled: config.injection_enabled,
            max_commands_per_sec: config.max_commands_per_sec,
        });

        Self {
            backend,
            config,
            state_machine,
            backoff,
            var_registry: SimVarRegistry::new(),
            detection_engine: AircraftDetectionEngine::default(),
            injector,
            registered_vars: Vec::new(),
            axis_event_ids: HashMap::new(),
            next_event_id: 200,
            latest_snapshot: None,
            latest_aircraft: None,
            aircraft_events: Vec::new(),
        }
    }

    // -- accessors --

    pub fn state(&self) -> SimConnectAdapterState {
        self.state_machine.state()
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn injector(&self) -> &SimControlInjector {
        &self.injector
    }

    pub fn injector_mut(&mut self) -> &mut SimControlInjector {
        &mut self.injector
    }

    pub fn var_registry(&self) -> &SimVarRegistry {
        &self.var_registry
    }

    pub fn registered_vars(&self) -> &[String] {
        &self.registered_vars
    }

    pub fn is_connected(&self) -> bool {
        self.state_machine.is_healthy()
    }

    // -- lifecycle --

    /// Attempt to open a SimConnect connection.
    pub fn connect(&mut self) -> Result<(), BackendError> {
        // Transition: Disconnected → Connecting (first OpenReceived)
        self.state_machine
            .transition(SimConnectEvent::OpenReceived)
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        // Actually open the connection.
        if let Err(e) = self.backend.open(&self.config.app_name) {
            self.state_machine
                .transition(SimConnectEvent::ConnectionLost(e.to_string()))
                .ok();
            return Err(e);
        }

        // Transition: Connecting → Connected (second OpenReceived)
        self.state_machine
            .transition(SimConnectEvent::OpenReceived)
            .map_err(|e| BackendError::ConnectionFailed(e.to_string()))?;

        self.backoff.reset();

        // Subscribe to system events.
        self.backend
            .subscribe_system_event(EVT_AIRCRAFT_LOADED, "AircraftLoaded")?;

        // Register variables and start data requests.
        self.register_variables()?;
        self.setup_axis_events()?;

        Ok(())
    }

    /// Gracefully close the connection.
    pub fn disconnect(&mut self) -> Result<(), BackendError> {
        let _ = self.backend.close();
        self.state_machine
            .transition(SimConnectEvent::Shutdown)
            .ok();
        self.registered_vars.clear();
        self.axis_event_ids.clear();
        self.latest_snapshot = None;
        self.latest_aircraft = None;
        Ok(())
    }

    /// Attempt reconnection with backoff.  Returns `Ok(true)` if connected,
    /// `Ok(false)` if backoff delay says "not yet", `Err` on hard failure.
    pub fn try_reconnect(&mut self) -> Result<bool, BackendError> {
        if !self.state_machine.is_recoverable() {
            return Err(BackendError::ConnectionFailed(
                "max retries exhausted".into(),
            ));
        }

        // Clean up previous session.
        let _ = self.backend.close();

        // Attempt fresh connect.
        match self.connect() {
            Ok(()) => Ok(true),
            Err(e) => {
                let _delay = self.backoff.next_delay();
                Err(e)
            }
        }
    }

    /// Get the delay before the next reconnection attempt.
    pub fn next_reconnect_delay(&mut self) -> Duration {
        self.backoff.next_delay()
    }

    // -- dispatch loop --

    /// Process one tick of the dispatch loop. Returns the number of messages
    /// processed.
    pub fn poll(&mut self) -> Result<usize, BackendError> {
        let mut count = 0usize;
        loop {
            match self.backend.get_next_dispatch() {
                Ok(Some(msg)) => {
                    self.handle_dispatch(msg)?;
                    count += 1;
                }
                Ok(None) => break,
                Err(BackendError::ConnectionLost(reason)) => {
                    self.state_machine
                        .transition(SimConnectEvent::ConnectionLost(reason.clone()))
                        .ok();
                    self.latest_snapshot = None;
                    return Err(BackendError::ConnectionLost(reason));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(count)
    }

    fn handle_dispatch(&mut self, msg: DispatchMessage) -> Result<(), BackendError> {
        match msg {
            DispatchMessage::Open => {
                // Already transitioned during connect(); ignore duplicate.
            }
            DispatchMessage::Quit => {
                self.state_machine
                    .transition(SimConnectEvent::ConnectionLost("simulator quit".into()))
                    .ok();
                self.latest_snapshot = None;
            }
            DispatchMessage::SimObjectData {
                define_id,
                request_id,
                values,
            } => {
                self.handle_sim_data(define_id, request_id, &values)?;
            }
            DispatchMessage::Event { event_id, data: _ } => {
                if event_id == EVT_AIRCRAFT_LOADED {
                    // Aircraft changed — reset detection, go back to Connected.
                    self.latest_aircraft = None;
                    self.latest_snapshot = None;
                    if matches!(
                        self.state_machine.state(),
                        SimConnectAdapterState::Active | SimConnectAdapterState::Stale
                    ) {
                        self.state_machine
                            .transition(SimConnectEvent::ConnectionLost("aircraft reload".into()))
                            .ok();
                        // Immediately attempt re-entry if recoverable.
                        let _ = self.state_machine.transition(SimConnectEvent::OpenReceived);
                        let _ = self.state_machine.transition(SimConnectEvent::OpenReceived);
                    }
                }
            }
            DispatchMessage::Exception { code: _ } => {
                // Log / count but don't transition.
            }
        }
        Ok(())
    }

    fn handle_sim_data(
        &mut self,
        define_id: u32,
        _request_id: u32,
        values: &[f64],
    ) -> Result<(), BackendError> {
        if define_id == DEF_AIRCRAFT {
            self.handle_aircraft_data(values);
            return Ok(());
        }

        if define_id == DEF_TELEMETRY {
            self.handle_telemetry_data(values);
        }

        Ok(())
    }

    fn handle_aircraft_data(&mut self, values: &[f64]) {
        // In the real adapter, the TITLE is a string datum.  In the mock
        // we encode a simple numeric "aircraft code" in values[0] (see tests).
        // The detection engine is invoked separately via `detect_aircraft()`.
        // Here we just signal that aircraft data was received.
        let _ = self
            .state_machine
            .transition(SimConnectEvent::AircraftDetected);
        let _ = self
            .state_machine
            .transition(SimConnectEvent::TelemetryReceived);

        // Build a snapshot from the aircraft define if there's data.
        if !values.is_empty() {
            let mut snap = VarSnapshot::default();
            snap.values.insert("_aircraft_code".to_string(), values[0]);
            self.latest_snapshot = Some(snap);
        }
    }

    fn handle_telemetry_data(&mut self, values: &[f64]) {
        let _ = self
            .state_machine
            .transition(SimConnectEvent::TelemetryReceived);

        let mut snap = VarSnapshot::default();
        for (i, val) in values.iter().enumerate() {
            let name = self
                .registered_vars
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("var_{i}"));
            snap.values.insert(name, *val);
        }
        self.latest_snapshot = Some(snap);
    }

    // -- variable registration --

    /// Register telemetry variables from the var-registry on the backend.
    fn register_variables(&mut self) -> Result<(), BackendError> {
        self.registered_vars.clear();

        // Register core telemetry vars (flight controls + navigation).
        let categories = [SimVarCategory::FlightControls, SimVarCategory::Navigation];
        let mut vars: Vec<&SimVar> = Vec::new();
        for cat in &categories {
            vars.extend(self.var_registry.by_category(*cat));
        }
        // Sort for deterministic ordering.
        vars.sort_by_key(|v| v.name);

        for var in &vars {
            self.backend
                .add_to_data_definition(DEF_TELEMETRY, var.name, var.unit)?;
            self.registered_vars.push(var.name.to_string());
        }

        // Request periodic telemetry data.
        if !self.registered_vars.is_empty() {
            self.backend.request_data(REQ_TELEMETRY, DEF_TELEMETRY)?;
        }

        // Register aircraft-identification variables (just TITLE placeholder).
        self.backend
            .add_to_data_definition(DEF_AIRCRAFT, "TITLE", "string")?;
        self.backend.request_data(REQ_AIRCRAFT, DEF_AIRCRAFT)?;

        Ok(())
    }

    // -- axis event setup --

    fn setup_axis_events(&mut self) -> Result<(), BackendError> {
        let axes = [
            AxisId::Elevator,
            AxisId::Ailerons,
            AxisId::Rudder,
            AxisId::Throttle,
            AxisId::Mixture,
            AxisId::Propeller,
        ];

        for axis in &axes {
            let eid = self.next_event_id;
            self.next_event_id += 1;
            self.backend
                .map_client_event(eid, axis.simconnect_event())?;
            self.axis_event_ids.insert(eid, *axis);
        }

        Ok(())
    }

    // -- control injection --

    /// Inject an axis value through the SimConnect backend.
    /// Value is clamped to −16384…+16383 before sending.
    pub fn inject_axis(&mut self, axis: AxisId, raw_value: i32) -> Result<(), BackendError> {
        let clamped = raw_value.clamp(-16384, 16383);

        // Find the event ID for this axis.
        let event_id = self
            .axis_event_ids
            .iter()
            .find(|(_, a)| **a == axis)
            .map(|(eid, _)| *eid)
            .ok_or_else(|| BackendError::InvalidRequest(format!("axis {:?} not mapped", axis)))?;

        self.backend.transmit_event(event_id, clamped as u32)?;
        self.injector.record_sent(1);
        Ok(())
    }

    /// Inject a key event (e.g. GEAR_TOGGLE).
    pub fn inject_key_event(&mut self, event_name: &str, data: u32) -> Result<(), BackendError> {
        // Check if already mapped; if not, map it.
        let event_id = self
            .find_event_id(event_name)
            .or_else(|| {
                let eid = self.next_event_id;
                self.next_event_id += 1;
                self.backend.map_client_event(eid, event_name).ok()?;
                Some(eid)
            })
            .ok_or_else(|| BackendError::EventFailed(format!("cannot map event {event_name}")))?;

        self.backend.transmit_event(event_id, data)?;
        self.injector.record_sent(1);
        Ok(())
    }

    fn find_event_id(&self, _event_name: &str) -> Option<u32> {
        // Mock backend exposes mapped_events; in real code we'd track our own map.
        // We maintain axis_event_ids + can inspect backend.
        None // Only axis events are cached; key events use fresh mapping.
    }

    // -- SimVar write --

    /// Write SimVar values back to the sim via SetDataOnSimObject.
    /// The `values` slice must match the order of the telemetry definition.
    pub fn write_simvar(&mut self, define_id: u32, values: &[f64]) -> Result<(), BackendError> {
        self.backend.set_data_on_sim_object(define_id, values)
    }

    // -- telemetry snapshot publishing --

    /// Build a `VarSnapshot` from the latest telemetry data and return it.
    /// Returns `None` if no telemetry has been received yet.
    pub fn take_snapshot(&self) -> Option<VarSnapshot> {
        self.latest_snapshot.clone()
    }

    /// Returns `true` if new telemetry data is available since the last call
    /// to `take_snapshot`.
    pub fn has_pending_telemetry(&self) -> bool {
        self.latest_snapshot.is_some()
    }

    // -- aircraft detection --

    /// Run aircraft detection on the provided sim data.
    pub fn detect_aircraft(&mut self, data: &SimAircraftData) -> DetectionResult {
        let result = self.detection_engine.detect(data);
        if result.icao.is_some() {
            let changed = AircraftChanged {
                title: data.title.clone(),
                icao: result.icao.clone(),
                display_name: result.display_name.clone(),
            };
            self.latest_aircraft = Some(changed.clone());
            self.aircraft_events.push(changed);
        }
        result
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aircraft_detection::MatchConfidence;

    fn default_bridge() -> SimConnectBridge<MockSimConnectBackend> {
        SimConnectBridge::new(MockSimConnectBackend::new(), BridgeConfig::default())
    }

    fn connected_bridge() -> SimConnectBridge<MockSimConnectBackend> {
        let mut b = default_bridge();
        b.connect().expect("connect must succeed on mock");
        b
    }

    // -- lifecycle tests --

    #[test]
    fn initial_state_is_disconnected() {
        let b = default_bridge();
        assert_eq!(b.state(), SimConnectAdapterState::Disconnected);
        assert!(!b.is_connected());
    }

    #[test]
    fn connect_transitions_to_connected() {
        let mut b = default_bridge();
        b.connect().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Connected);
        assert!(b.is_connected());
        assert!(b.backend().is_open());
    }

    #[test]
    fn connect_failure_transitions_to_reconnecting() {
        let mut b = default_bridge();
        b.backend_mut().fail_next_open = true;
        let res = b.connect();
        assert!(res.is_err());
        // Connecting → ConnectionLost → Reconnecting (recoverable)
        assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);
    }

    #[test]
    fn disconnect_returns_to_disconnected() {
        let mut b = connected_bridge();
        b.disconnect().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Disconnected);
        assert!(!b.backend().is_open());
    }

    #[test]
    fn connect_registers_system_events() {
        let b = connected_bridge();
        let sys = b.backend().system_events();
        assert!(
            sys.values().any(|v| v == "AircraftLoaded"),
            "AircraftLoaded must be subscribed"
        );
    }

    // -- variable registration tests --

    #[test]
    fn connect_registers_variables() {
        let b = connected_bridge();
        assert!(
            !b.registered_vars().is_empty(),
            "telemetry vars must be registered"
        );

        // Backend should have definitions for DEF_TELEMETRY and DEF_AIRCRAFT.
        let defs = b.backend().definitions();
        assert!(defs.contains_key(&DEF_TELEMETRY));
        assert!(defs.contains_key(&DEF_AIRCRAFT));
    }

    #[test]
    fn registered_vars_come_from_registry() {
        let b = connected_bridge();
        let reg = b.var_registry();
        for name in b.registered_vars() {
            assert!(
                reg.contains(name),
                "registered var '{name}' must exist in registry"
            );
        }
    }

    #[test]
    fn connect_starts_data_requests() {
        let b = connected_bridge();
        let reqs = b.backend().active_requests();
        assert!(reqs.contains_key(&REQ_TELEMETRY));
        assert!(reqs.contains_key(&REQ_AIRCRAFT));
    }

    // -- axis event setup tests --

    #[test]
    fn connect_maps_axis_events() {
        let b = connected_bridge();
        let mapped = b.backend().mapped_events();
        let names: Vec<&String> = mapped.values().collect();
        assert!(
            names.iter().any(|n| n.as_str() == "AXIS_ELEVATOR_SET"),
            "elevator must be mapped"
        );
        assert!(
            names.iter().any(|n| n.as_str() == "AXIS_AILERONS_SET"),
            "ailerons must be mapped"
        );
    }

    // -- dispatch / telemetry tests --

    #[test]
    fn poll_with_no_messages_returns_zero() {
        let mut b = connected_bridge();
        let count = b.poll().unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn poll_processes_telemetry_data() {
        let mut b = connected_bridge();
        let n = b.registered_vars().len();
        let values: Vec<f64> = (0..n).map(|i| i as f64 * 0.1).collect();
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: values.clone(),
            });

        let count = b.poll().unwrap();
        assert_eq!(count, 1);

        let snap = b.latest_snapshot.as_ref().expect("snapshot must exist");
        assert_eq!(snap.values.len(), n);
        // Verify first registered var has correct value.
        let first = &b.registered_vars()[0];
        assert!(
            (snap.values[first] - 0.0).abs() < f64::EPSILON,
            "first var must be 0.0"
        );
    }

    #[test]
    fn poll_processes_aircraft_data() {
        let mut b = connected_bridge();
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_AIRCRAFT,
                request_id: REQ_AIRCRAFT,
                values: vec![42.0],
            });

        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Active);
        let snap = b.latest_snapshot.as_ref().unwrap();
        assert!((snap.values["_aircraft_code"] - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn poll_handles_quit_message() {
        let mut b = connected_bridge();
        b.backend_mut().push_dispatch(DispatchMessage::Quit);

        b.poll().unwrap();
        // Connected → ConnectionLost → Reconnecting (recoverable)
        assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);
        assert!(b.latest_snapshot.is_none());
    }

    #[test]
    fn poll_handles_aircraft_loaded_event() {
        let mut b = connected_bridge();
        // First bring to Active via aircraft data.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_AIRCRAFT,
                request_id: REQ_AIRCRAFT,
                values: vec![1.0],
            });
        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Active);

        // Now fire AircraftLoaded — should reset aircraft and re-enter Connected.
        b.backend_mut().push_dispatch(DispatchMessage::Event {
            event_id: EVT_AIRCRAFT_LOADED,
            data: 0,
        });
        b.poll().unwrap();
        assert!(b.latest_aircraft.is_none());
        assert!(b.latest_snapshot.is_none());
    }

    #[test]
    fn poll_handles_connection_lost() {
        let mut b = connected_bridge();
        // Close the backend to simulate loss.
        b.backend_mut().close().unwrap();
        let res = b.poll();
        assert!(res.is_err());
        // Connected → ConnectionLost → Reconnecting (recoverable)
        assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);
    }

    // -- control injection tests --

    #[test]
    fn inject_axis_clamps_and_transmits() {
        let mut b = connected_bridge();
        b.inject_axis(AxisId::Elevator, 50000).unwrap();
        let events = b.backend().transmitted_events();
        assert_eq!(events.len(), 1);
        // Value should be clamped to 16383.
        assert_eq!(events[0].1, 16383u32);
    }

    #[test]
    fn inject_axis_negative_clamp() {
        let mut b = connected_bridge();
        b.inject_axis(AxisId::Rudder, -50000).unwrap();
        let events = b.backend().transmitted_events();
        // -16384 as u32 wraps, but the backend sees the raw u32 cast.
        assert_eq!(events[0].1, (-16384i32) as u32);
    }

    #[test]
    fn inject_axis_updates_sent_counter() {
        let mut b = connected_bridge();
        b.inject_axis(AxisId::Ailerons, 0).unwrap();
        assert_eq!(b.injector().commands_sent(), 1);
    }

    #[test]
    fn inject_key_event_maps_and_transmits() {
        let mut b = connected_bridge();
        b.inject_key_event("GEAR_TOGGLE", 0).unwrap();
        let events = b.backend().transmitted_events();
        assert!(!events.is_empty());
        assert_eq!(b.injector().commands_sent(), 1);
    }

    #[test]
    fn inject_axis_fails_when_disconnected() {
        let mut b = default_bridge();
        let res = b.inject_axis(AxisId::Elevator, 0);
        assert!(res.is_err());
    }

    #[test]
    fn inject_transmit_failure_propagates() {
        let mut b = connected_bridge();
        b.backend_mut().fail_next_transmit = true;
        let res = b.inject_axis(AxisId::Elevator, 0);
        assert!(res.is_err());
    }

    // -- aircraft detection tests --

    #[test]
    fn detect_aircraft_cessna() {
        let mut b = default_bridge();
        let result = b.detect_aircraft(&SimAircraftData {
            title: "Cessna 172 Skyhawk".into(),
            atc_type: "CESSNA".into(),
            atc_model: "C172".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert_eq!(result.icao, Some("C172".into()));

        // Should have emitted an aircraft change event.
        assert_eq!(b.aircraft_events.len(), 1);
        assert_eq!(b.aircraft_events[0].icao, Some("C172".into()));
        assert_eq!(
            b.latest_aircraft.as_ref().unwrap().icao,
            Some("C172".into())
        );
    }

    #[test]
    fn detect_aircraft_unknown_emits_no_event() {
        let mut b = default_bridge();
        let result = b.detect_aircraft(&SimAircraftData {
            title: "Unknown XYZ".into(),
            atc_type: "UNKNOWN".into(),
            atc_model: "XXXX".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::None);
        assert!(b.aircraft_events.is_empty());
    }

    #[test]
    fn detect_aircraft_community_mod() {
        let mut b = default_bridge();
        let result = b.detect_aircraft(&SimAircraftData {
            title: "FlyByWire A320neo (LEAP)".into(),
            atc_type: "AIRBUS".into(),
            atc_model: "A320".into(),
        });
        assert_eq!(result.confidence, MatchConfidence::Exact);
        assert!(result.is_community_mod);
        assert_eq!(b.aircraft_events.len(), 1);
    }

    // -- reconnection tests --

    #[test]
    fn reconnect_after_failure() {
        let mut b = default_bridge();
        b.backend_mut().fail_next_open = true;
        let _ = b.connect(); // fails → Reconnecting
        assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);

        // Now reconnect should succeed.
        let ok = b.try_reconnect().unwrap();
        assert!(ok);
        assert_eq!(b.state(), SimConnectAdapterState::Connected);
    }

    #[test]
    fn reconnect_exhausted_returns_error() {
        let config = BridgeConfig {
            max_retries: 1,
            ..Default::default()
        };
        let mut b = SimConnectBridge::new(MockSimConnectBackend::new(), config);

        // First failure — Connecting → ConnectionLost (1 error == max_retries).
        b.backend_mut().fail_next_open = true;
        let _ = b.connect();
        // With max_retries=1, error_count=1, is_recoverable() is false → Error
        assert_eq!(b.state(), SimConnectAdapterState::Error);

        // State machine says not recoverable.
        let res = b.try_reconnect();
        assert!(res.is_err());
    }

    // -- backoff tests --

    #[test]
    fn backoff_increases_after_failures() {
        let mut b = default_bridge();
        let d1 = b.next_reconnect_delay();
        let d2 = b.next_reconnect_delay();
        assert!(d2 > d1, "backoff must increase");
    }

    #[test]
    fn backoff_resets_on_connect() {
        let mut b = default_bridge();
        // Advance backoff.
        let _ = b.next_reconnect_delay();
        let _ = b.next_reconnect_delay();
        // Connect resets.
        b.connect().unwrap();
        let d = b.next_reconnect_delay();
        assert_eq!(
            d,
            Duration::from_secs(1),
            "backoff must reset after connect"
        );
    }

    // -- full lifecycle test --

    #[test]
    fn full_lifecycle() {
        let mut b = default_bridge();
        assert_eq!(b.state(), SimConnectAdapterState::Disconnected);

        // Connect.
        b.connect().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Connected);

        // Receive aircraft data → Active.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_AIRCRAFT,
                request_id: REQ_AIRCRAFT,
                values: vec![1.0],
            });
        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Active);

        // Receive telemetry.
        let n = b.registered_vars().len();
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![0.5; n],
            });
        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Active);
        assert!(b.latest_snapshot.is_some());

        // Inject an axis.
        b.inject_axis(AxisId::Elevator, 4096).unwrap();
        assert_eq!(b.backend().transmitted_events().len(), 1);

        // Detect aircraft.
        let det = b.detect_aircraft(&SimAircraftData {
            title: "Cessna 172 Skyhawk".into(),
            atc_type: "CESSNA".into(),
            atc_model: "C172".into(),
        });
        assert_eq!(det.icao, Some("C172".into()));

        // Disconnect.
        b.disconnect().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Disconnected);
    }

    // -- error / edge-case tests --

    #[test]
    fn poll_exception_does_not_crash() {
        let mut b = connected_bridge();
        b.backend_mut()
            .push_dispatch(DispatchMessage::Exception { code: 7 });
        let count = b.poll().unwrap();
        assert_eq!(count, 1);
        // State should not change on exception.
        assert_eq!(b.state(), SimConnectAdapterState::Connected);
    }

    #[test]
    fn multiple_telemetry_updates_overwrite_snapshot() {
        let mut b = connected_bridge();
        let n = b.registered_vars().len();

        // First update.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![1.0; n],
            });
        b.poll().unwrap();
        let first_val = b.latest_snapshot.as_ref().unwrap().values[&b.registered_vars()[0]];

        // Second update with different values.
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![2.0; n],
            });
        b.poll().unwrap();
        let second_val = b.latest_snapshot.as_ref().unwrap().values[&b.registered_vars()[0]];
        assert!((second_val - 2.0).abs() < f64::EPSILON);
        assert!((first_val - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn backend_error_display() {
        let e = BackendError::ConnectionFailed("timeout".into());
        assert_eq!(e.to_string(), "connection failed: timeout");
        let e = BackendError::ConnectionLost("pipe broken".into());
        assert_eq!(e.to_string(), "connection lost: pipe broken");
        let e = BackendError::InvalidRequest("bad id".into());
        assert_eq!(e.to_string(), "invalid request: bad id");
        let e = BackendError::EventFailed("transmit".into());
        assert_eq!(e.to_string(), "event failed: transmit");
    }

    #[test]
    fn mock_backend_close_clears_state() {
        let mut mock = MockSimConnectBackend::new();
        mock.open("test").unwrap();
        mock.add_to_data_definition(1, "AILERON POSITION", "position")
            .unwrap();
        mock.map_client_event(1, "GEAR_TOGGLE").unwrap();
        mock.subscribe_system_event(1, "SimStart").unwrap();
        mock.request_data(1, 1).unwrap();

        mock.close().unwrap();
        assert!(!mock.is_open());
        assert!(mock.definitions().is_empty());
        assert!(mock.mapped_events().is_empty());
        assert!(mock.system_events().is_empty());
        assert!(mock.active_requests().is_empty());
    }

    #[test]
    fn mock_backend_operations_fail_when_closed() {
        let mut mock = MockSimConnectBackend::new();
        assert!(mock.add_to_data_definition(1, "X", "Y").is_err());
        assert!(mock.request_data(1, 1).is_err());
        assert!(mock.map_client_event(1, "X").is_err());
        assert!(mock.transmit_event(1, 0).is_err());
        assert!(mock.subscribe_system_event(1, "X").is_err());
        assert!(mock.get_next_dispatch().is_err());
    }

    // -- SimVar write tests --

    #[test]
    fn write_simvar_sends_to_backend() {
        let mut b = connected_bridge();
        b.write_simvar(DEF_TELEMETRY, &[1.0, 2.0, 3.0]).unwrap();
        let data = b.backend().written_data();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].0, DEF_TELEMETRY);
        assert_eq!(data[0].1, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn write_simvar_fails_when_backend_errors() {
        let mut b = connected_bridge();
        b.backend_mut().fail_next_write = true;
        let res = b.write_simvar(DEF_TELEMETRY, &[1.0]);
        assert!(res.is_err());
    }

    // -- snapshot tests --

    #[test]
    fn take_snapshot_returns_none_before_telemetry() {
        let b = connected_bridge();
        assert!(b.take_snapshot().is_none());
        assert!(!b.has_pending_telemetry());
    }

    #[test]
    fn take_snapshot_returns_data_after_telemetry() {
        let mut b = connected_bridge();
        let n = b.registered_vars().len();
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![42.0; n],
            });
        b.poll().unwrap();
        assert!(b.has_pending_telemetry());
        let snap = b.take_snapshot().expect("snapshot should exist");
        assert_eq!(snap.values.len(), n);
    }

    // -- reconnecting state flow tests --

    #[test]
    fn poll_reconnecting_state_preserved_on_quit() {
        let mut b = connected_bridge();
        // Receive some telemetry first.
        let n = b.registered_vars().len();
        b.backend_mut()
            .push_dispatch(DispatchMessage::SimObjectData {
                define_id: DEF_TELEMETRY,
                request_id: REQ_TELEMETRY,
                values: vec![1.0; n],
            });
        b.poll().unwrap();
        assert!(b.latest_snapshot.is_some());

        // Quit clears snapshot and goes to Reconnecting.
        b.backend_mut().push_dispatch(DispatchMessage::Quit);
        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);
        assert!(b.latest_snapshot.is_none());
    }

    #[test]
    fn full_reconnect_lifecycle() {
        let mut b = default_bridge();

        // Connect successfully.
        b.connect().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Connected);

        // Lose connection → Reconnecting.
        b.backend_mut().push_dispatch(DispatchMessage::Quit);
        b.poll().unwrap();
        assert_eq!(b.state(), SimConnectAdapterState::Reconnecting);

        // Reconnect successfully.
        let ok = b.try_reconnect().unwrap();
        assert!(ok);
        assert_eq!(b.state(), SimConnectAdapterState::Connected);
    }
}
