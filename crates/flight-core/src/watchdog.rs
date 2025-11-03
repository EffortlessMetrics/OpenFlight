// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Watchdog and quarantine system for Flight Hub
//!
//! Implements comprehensive watchdog monitoring for USB endpoints, plugins,
//! and system components with automatic quarantine of failed components.
//! Provides synthetic fault injection for testing and validation.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Watchdog-specific errors
#[derive(Error, Debug)]
pub enum WatchdogError {
    #[error("Component quarantined: {component_id}")]
    ComponentQuarantined { component_id: String },

    #[error("USB endpoint timeout: {endpoint}")]
    UsbEndpointTimeout { endpoint: String },

    #[error("Plugin overrun: {plugin_id} exceeded {budget:?}")]
    PluginOverrun { plugin_id: String, budget: Duration },

    #[error("NaN guard triggered: {context}")]
    NanGuard { context: String },

    #[error("Watchdog system error: {0}")]
    System(String),
}

/// Types of components that can be monitored and quarantined
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComponentType {
    /// USB HID device endpoint
    UsbEndpoint(String),
    /// Native plugin process
    NativePlugin(String),
    /// WASM plugin instance
    WasmPlugin(String),
    /// Simulator adapter
    SimAdapter(String),
    /// Panel device
    PanelDevice(String),
    /// Axis processing node
    AxisNode(String),
}

impl ComponentType {
    /// Get human-readable name for this component type
    pub fn display_name(&self) -> String {
        match self {
            ComponentType::UsbEndpoint(id) => format!("USB Endpoint {}", id),
            ComponentType::NativePlugin(id) => format!("Native Plugin {}", id),
            ComponentType::WasmPlugin(id) => format!("WASM Plugin {}", id),
            ComponentType::SimAdapter(id) => format!("Sim Adapter {}", id),
            ComponentType::PanelDevice(id) => format!("Panel Device {}", id),
            ComponentType::AxisNode(id) => format!("Axis Node {}", id),
        }
    }

    /// Get component ID string
    pub fn id(&self) -> &str {
        match self {
            ComponentType::UsbEndpoint(id)
            | ComponentType::NativePlugin(id)
            | ComponentType::WasmPlugin(id)
            | ComponentType::SimAdapter(id)
            | ComponentType::PanelDevice(id)
            | ComponentType::AxisNode(id) => id,
        }
    }
}

/// Watchdog monitoring configuration for a component
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// Maximum allowed execution time per tick
    pub max_execution_time: Duration,
    /// Timeout for USB operations
    pub usb_timeout: Duration,
    /// Maximum consecutive failures before quarantine
    pub max_consecutive_failures: u32,
    /// Time window for failure rate calculation
    pub failure_rate_window: Duration,
    /// Maximum failures per window before quarantine
    pub max_failures_per_window: u32,
    /// Whether to enable NaN guards
    pub enable_nan_guards: bool,
    /// Whether component is critical (affects quarantine behavior)
    pub is_critical: bool,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            max_execution_time: Duration::from_micros(100), // 100μs budget for plugins
            usb_timeout: Duration::from_millis(100),        // 100ms USB timeout
            max_consecutive_failures: 3,
            failure_rate_window: Duration::from_secs(60),
            max_failures_per_window: 10,
            enable_nan_guards: true,
            is_critical: false,
        }
    }
}

/// Quarantine status of a component
#[derive(Debug, Clone, PartialEq)]
pub enum QuarantineStatus {
    /// Component is healthy and active
    Active,
    /// Component is quarantined for the session
    Quarantined {
        reason: String,
        quarantined_at: Instant,
        failure_count: u32,
    },
    /// Component is temporarily disabled for recovery
    Recovering { until: Instant, attempt_count: u32 },
}

/// Record of a watchdog event
#[derive(Debug, Clone)]
pub struct WatchdogEvent {
    /// When the event occurred
    pub timestamp: Instant,
    /// Component that triggered the event
    pub component: ComponentType,
    /// Type of event
    pub event_type: WatchdogEventType,
    /// Additional context
    pub context: String,
    /// Execution time if applicable
    pub execution_time: Option<Duration>,
    /// Action taken in response
    pub action_taken: WatchdogAction,
}

/// Types of watchdog events
#[derive(Debug, Clone, PartialEq)]
pub enum WatchdogEventType {
    /// Plugin exceeded time budget
    PluginOverrun,
    /// USB endpoint timeout
    UsbTimeout,
    /// USB endpoint error
    UsbError,
    /// NaN value detected
    NanDetected,
    /// Component recovered from failure
    ComponentRecovered,
    /// Component quarantined
    ComponentQuarantined,
    /// Synthetic fault injected (testing)
    SyntheticFault,
}

/// Actions taken by watchdog in response to events
#[derive(Debug, Clone, PartialEq)]
pub enum WatchdogAction {
    /// Log event and continue
    LogOnly,
    /// Quarantine component for session
    QuarantineComponent,
    /// Attempt component recovery
    AttemptRecovery,
    /// Trigger emergency stop
    EmergencyStop,
    /// Reset USB endpoint
    ResetUsbEndpoint,
}

/// USB endpoint monitoring state
#[derive(Debug)]
struct UsbEndpointState {
    /// Last successful operation timestamp
    last_success: Instant,
    /// Consecutive failure count
    consecutive_failures: u32,
    /// Recent failure timestamps
    recent_failures: VecDeque<Instant>,
    /// Whether endpoint is currently stalled
    is_stalled: bool,
    /// Stall frame counter
    stall_frame_count: u32,
}

/// Plugin execution monitoring state
#[derive(Debug)]
struct PluginState {
    /// Recent execution times
    execution_times: VecDeque<(Instant, Duration)>,
    /// Consecutive overrun count
    consecutive_overruns: u32,
    /// Total overrun count
    total_overruns: u32,
    /// Last execution timestamp
    last_execution: Option<Instant>,
}

/// Component monitoring and quarantine system
#[derive(Debug)]
pub struct WatchdogSystem {
    /// Configuration for each component type
    configs: HashMap<ComponentType, WatchdogConfig>,
    /// Quarantine status for each component
    quarantine_status: HashMap<ComponentType, QuarantineStatus>,
    /// USB endpoint monitoring state
    usb_endpoints: HashMap<String, UsbEndpointState>,
    /// Plugin monitoring state
    plugins: HashMap<String, PluginState>,
    /// Event history
    event_history: VecDeque<WatchdogEvent>,
    /// Maximum event history size
    max_event_history: usize,
    /// Synthetic fault injection state
    fault_injection: Option<FaultInjectionState>,
    /// USB frame stall counter
    usb_stall_counter: u32,
    /// Endpoint wedge detection timer
    endpoint_wedge_timer: Option<Instant>,
}

/// Synthetic fault injection for testing
#[derive(Debug)]
struct FaultInjectionState {
    /// Faults to inject
    pending_faults: VecDeque<SyntheticFault>,
    /// Whether injection is enabled
    enabled: bool,
}

/// Synthetic fault for testing
#[derive(Debug, Clone)]
pub struct SyntheticFault {
    /// Target component
    pub component: ComponentType,
    /// Type of fault to inject
    pub fault_type: WatchdogEventType,
    /// When to inject the fault
    pub inject_at: Instant,
    /// Additional context
    pub context: String,
}

impl WatchdogSystem {
    /// Create new watchdog system
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            quarantine_status: HashMap::new(),
            usb_endpoints: HashMap::new(),
            plugins: HashMap::new(),
            event_history: VecDeque::new(),
            max_event_history: 10000,
            fault_injection: None,
            usb_stall_counter: 0,
            endpoint_wedge_timer: None,
        }
    }

    /// Register a component for monitoring
    pub fn register_component(&mut self, component: ComponentType, config: WatchdogConfig) {
        debug!(
            "Registering component for watchdog monitoring: {}",
            component.display_name()
        );

        self.configs.insert(component.clone(), config);
        self.quarantine_status
            .insert(component.clone(), QuarantineStatus::Active);

        // Initialize component-specific state
        match &component {
            ComponentType::UsbEndpoint(id) => {
                self.usb_endpoints.insert(
                    id.clone(),
                    UsbEndpointState {
                        last_success: Instant::now(),
                        consecutive_failures: 0,
                        recent_failures: VecDeque::new(),
                        is_stalled: false,
                        stall_frame_count: 0,
                    },
                );
            }
            ComponentType::NativePlugin(id) | ComponentType::WasmPlugin(id) => {
                self.plugins.insert(
                    id.clone(),
                    PluginState {
                        execution_times: VecDeque::new(),
                        consecutive_overruns: 0,
                        total_overruns: 0,
                        last_execution: None,
                    },
                );
            }
            _ => {} // Other component types don't need special state
        }
    }

    /// Unregister a component from monitoring
    pub fn unregister_component(&mut self, component: &ComponentType) {
        debug!(
            "Unregistering component from watchdog monitoring: {}",
            component.display_name()
        );

        self.configs.remove(component);
        self.quarantine_status.remove(component);

        match component {
            ComponentType::UsbEndpoint(id) => {
                self.usb_endpoints.remove(id);
            }
            ComponentType::NativePlugin(id) | ComponentType::WasmPlugin(id) => {
                self.plugins.remove(id);
            }
            _ => {}
        }
    }

    /// Check if a component is quarantined
    pub fn is_quarantined(&self, component: &ComponentType) -> bool {
        matches!(
            self.quarantine_status.get(component),
            Some(QuarantineStatus::Quarantined { .. })
        )
    }

    /// Get quarantine status for a component
    pub fn get_quarantine_status(&self, component: &ComponentType) -> Option<&QuarantineStatus> {
        self.quarantine_status.get(component)
    }

    /// Get all quarantined components
    pub fn get_quarantined_components(&self) -> Vec<ComponentType> {
        self.quarantine_status
            .iter()
            .filter_map(|(component, status)| {
                if matches!(status, QuarantineStatus::Quarantined { .. }) {
                    Some(component.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Record USB endpoint success
    pub fn record_usb_success(&mut self, endpoint_id: &str) {
        if let Some(state) = self.usb_endpoints.get_mut(endpoint_id) {
            state.last_success = Instant::now();
            state.consecutive_failures = 0;
            state.is_stalled = false;
            state.stall_frame_count = 0;
        }
    }

    /// Record USB endpoint stall
    pub fn record_usb_stall(&mut self, endpoint_id: &str) -> Option<WatchdogEvent> {
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());

        if let Some(state) = self.usb_endpoints.get_mut(endpoint_id) {
            state.stall_frame_count += 1;
            state.is_stalled = true;

            // Trigger fault after 3 stalls (as per requirements)
            if state.stall_frame_count >= 3 {
                warn!(
                    "USB endpoint {} stalled for {} frames",
                    endpoint_id, state.stall_frame_count
                );

                let event = WatchdogEvent {
                    timestamp: Instant::now(),
                    component: component.clone(),
                    event_type: WatchdogEventType::UsbTimeout,
                    context: format!(
                        "USB stall detected after {} frames",
                        state.stall_frame_count
                    ),
                    execution_time: None,
                    action_taken: WatchdogAction::ResetUsbEndpoint,
                };

                // Reset counter before recording event to avoid borrowing issues
                state.stall_frame_count = 0;

                self.record_event(event.clone());
                return Some(event);
            }
        }

        None
    }

    /// Reset USB stall counter (called on successful frame)
    pub fn reset_usb_stall_counter(&mut self) {
        self.usb_stall_counter = 0;
        // Also reset per-endpoint stall counters
        for state in self.usb_endpoints.values_mut() {
            state.stall_frame_count = 0;
            state.is_stalled = false;
        }
    }

    /// Check for endpoint wedge condition
    pub fn check_endpoint_wedge(&mut self, endpoint_responsive: bool) -> Option<WatchdogEvent> {
        if !endpoint_responsive {
            if self.endpoint_wedge_timer.is_none() {
                self.endpoint_wedge_timer = Some(Instant::now());
            } else if let Some(timer) = self.endpoint_wedge_timer
                && timer.elapsed() >= Duration::from_millis(100)
            {
                self.endpoint_wedge_timer = None;

                let event = WatchdogEvent {
                    timestamp: Instant::now(),
                    component: ComponentType::UsbEndpoint("wedged_endpoint".to_string()),
                    event_type: WatchdogEventType::UsbTimeout,
                    context: "Endpoint wedged - unresponsive for >100ms".to_string(),
                    execution_time: None,
                    action_taken: WatchdogAction::ResetUsbEndpoint,
                };

                self.record_event(event.clone());
                return Some(event);
            }
        } else {
            self.endpoint_wedge_timer = None;
        }
        None
    }

    /// Record USB endpoint error
    pub fn record_usb_error(&mut self, endpoint_id: &str, error_context: &str) -> WatchdogEvent {
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());

        if let Some(state) = self.usb_endpoints.get_mut(endpoint_id) {
            state.consecutive_failures += 1;
            state.recent_failures.push_back(Instant::now());

            // Keep only recent failures within the window
            if let Some(config) = self.configs.get(&component) {
                let cutoff = Instant::now() - config.failure_rate_window;
                while let Some(&front_time) = state.recent_failures.front() {
                    if front_time < cutoff {
                        state.recent_failures.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }

        error!("USB endpoint {} error: {}", endpoint_id, error_context);

        let action = if self.should_quarantine_component(&component) {
            self.quarantine_component(&component, format!("USB error: {}", error_context));
            WatchdogAction::QuarantineComponent
        } else {
            WatchdogAction::ResetUsbEndpoint
        };

        let event = WatchdogEvent {
            timestamp: Instant::now(),
            component,
            event_type: WatchdogEventType::UsbError,
            context: error_context.to_string(),
            execution_time: None,
            action_taken: action,
        };

        self.record_event(event.clone());
        event
    }

    /// Record plugin execution time and check for overruns
    pub fn record_plugin_execution(
        &mut self,
        plugin_id: &str,
        execution_time: Duration,
        is_native: bool,
    ) -> Option<WatchdogEvent> {
        let component = if is_native {
            ComponentType::NativePlugin(plugin_id.to_string())
        } else {
            ComponentType::WasmPlugin(plugin_id.to_string())
        };

        // Get config first to avoid borrowing conflicts
        let max_execution_time = self.configs.get(&component)?.max_execution_time;
        let is_overrun = execution_time > max_execution_time;

        if let Some(state) = self.plugins.get_mut(plugin_id) {
            state
                .execution_times
                .push_back((Instant::now(), execution_time));
            state.last_execution = Some(Instant::now());

            // Keep only recent execution times
            let cutoff = Instant::now() - Duration::from_secs(60);
            while let Some(&(timestamp, _)) = state.execution_times.front() {
                if timestamp < cutoff {
                    state.execution_times.pop_front();
                } else {
                    break;
                }
            }

            // Check for overrun
            if is_overrun {
                state.consecutive_overruns += 1;
                state.total_overruns += 1;

                warn!(
                    "Plugin {} overrun: {:?} > {:?} (consecutive: {})",
                    plugin_id, execution_time, max_execution_time, state.consecutive_overruns
                );

                let should_quarantine = self.should_quarantine_component(&component);

                let action = if should_quarantine {
                    self.quarantine_component(
                        &component,
                        format!("Plugin overrun: {:?}", execution_time),
                    );
                    WatchdogAction::QuarantineComponent
                } else {
                    WatchdogAction::LogOnly
                };

                let event = WatchdogEvent {
                    timestamp: Instant::now(),
                    component,
                    event_type: WatchdogEventType::PluginOverrun,
                    context: format!(
                        "Execution time {:?} exceeded budget {:?}",
                        execution_time, max_execution_time
                    ),
                    execution_time: Some(execution_time),
                    action_taken: action,
                };

                self.record_event(event.clone());
                return Some(event);
            } else {
                // Reset consecutive overruns on successful execution
                state.consecutive_overruns = 0;
            }
        }

        None
    }

    /// Check for NaN values and trigger guard if found
    pub fn check_nan_guard(
        &mut self,
        value: f32,
        context: &str,
        component: ComponentType,
    ) -> Option<WatchdogEvent> {
        if let Some(config) = self.configs.get(&component)
            && config.enable_nan_guards
            && (value.is_nan() || value.is_infinite())
        {
            error!(
                "NaN guard triggered for {}: {} = {}",
                component.display_name(),
                context,
                value
            );

            let action = if config.is_critical {
                WatchdogAction::EmergencyStop
            } else {
                WatchdogAction::LogOnly
            };

            let event = WatchdogEvent {
                timestamp: Instant::now(),
                component,
                event_type: WatchdogEventType::NanDetected,
                context: format!("{} = {}", context, value),
                execution_time: None,
                action_taken: action,
            };

            self.record_event(event.clone());
            return Some(event);
        }

        None
    }

    /// Check USB endpoint timeout
    pub fn check_usb_timeout(&mut self, endpoint_id: &str) -> Option<WatchdogEvent> {
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());

        // Get timeout config and check state separately to avoid borrowing conflicts
        let usb_timeout = self.configs.get(&component)?.usb_timeout;
        let elapsed_time = self.usb_endpoints.get(endpoint_id)?.last_success.elapsed();

        if elapsed_time > usb_timeout {
            warn!("USB endpoint {} timeout: {:?}", endpoint_id, elapsed_time);

            let should_quarantine = self.should_quarantine_component(&component);

            let action = if should_quarantine {
                self.quarantine_component(&component, "USB timeout".to_string());
                WatchdogAction::QuarantineComponent
            } else {
                WatchdogAction::ResetUsbEndpoint
            };

            let event = WatchdogEvent {
                timestamp: Instant::now(),
                component,
                event_type: WatchdogEventType::UsbTimeout,
                context: format!("Timeout after {:?}", elapsed_time),
                execution_time: None,
                action_taken: action,
            };

            self.record_event(event.clone());
            return Some(event);
        }

        None
    }

    /// Determine if a component should be quarantined based on failure patterns
    fn should_quarantine_component(&self, component: &ComponentType) -> bool {
        if let Some(config) = self.configs.get(component) {
            match component {
                ComponentType::UsbEndpoint(id) => {
                    if let Some(state) = self.usb_endpoints.get(id) {
                        // Quarantine if too many consecutive failures
                        if state.consecutive_failures >= config.max_consecutive_failures {
                            return true;
                        }

                        // Quarantine if failure rate is too high
                        if state.recent_failures.len() as u32 >= config.max_failures_per_window {
                            return true;
                        }
                    }
                }
                ComponentType::NativePlugin(id) | ComponentType::WasmPlugin(id) => {
                    if let Some(state) = self.plugins.get(id) {
                        // Quarantine if too many consecutive overruns
                        if state.consecutive_overruns >= config.max_consecutive_failures {
                            return true;
                        }

                        // Quarantine if overrun rate is too high
                        let recent_overruns = state
                            .execution_times
                            .iter()
                            .filter(|(_, duration)| *duration > config.max_execution_time)
                            .count() as u32;

                        if recent_overruns >= config.max_failures_per_window {
                            return true;
                        }
                    }
                }
                _ => {
                    // For other component types, use simple failure counting
                    // This would need to be implemented based on specific component needs
                }
            }
        }

        false
    }

    /// Quarantine a component
    fn quarantine_component(&mut self, component: &ComponentType, reason: String) {
        warn!(
            "Quarantining component {}: {}",
            component.display_name(),
            reason
        );

        let failure_count = match component {
            ComponentType::UsbEndpoint(id) => self
                .usb_endpoints
                .get(id)
                .map(|s| s.consecutive_failures)
                .unwrap_or(0),
            ComponentType::NativePlugin(id) | ComponentType::WasmPlugin(id) => {
                self.plugins.get(id).map(|s| s.total_overruns).unwrap_or(0)
            }
            _ => 0,
        };

        self.quarantine_status.insert(
            component.clone(),
            QuarantineStatus::Quarantined {
                reason,
                quarantined_at: Instant::now(),
                failure_count,
            },
        );

        let event = WatchdogEvent {
            timestamp: Instant::now(),
            component: component.clone(),
            event_type: WatchdogEventType::ComponentQuarantined,
            context: "Component quarantined due to excessive failures".to_string(),
            execution_time: None,
            action_taken: WatchdogAction::QuarantineComponent,
        };

        self.record_event(event);
    }

    /// Attempt to recover a quarantined component
    pub fn attempt_recovery(&mut self, component: &ComponentType) -> bool {
        if let Some(status) = self.quarantine_status.get_mut(component) {
            match status {
                QuarantineStatus::Quarantined { .. } => {
                    info!(
                        "Attempting recovery for component: {}",
                        component.display_name()
                    );

                    *status = QuarantineStatus::Recovering {
                        until: Instant::now() + Duration::from_secs(30), // 30s recovery period
                        attempt_count: 1,
                    };

                    let event = WatchdogEvent {
                        timestamp: Instant::now(),
                        component: component.clone(),
                        event_type: WatchdogEventType::ComponentRecovered,
                        context: "Recovery attempt initiated".to_string(),
                        execution_time: None,
                        action_taken: WatchdogAction::AttemptRecovery,
                    };

                    self.record_event(event);
                    return true;
                }
                QuarantineStatus::Recovering {
                    until,
                    attempt_count: _,
                } => {
                    if Instant::now() > *until {
                        // Recovery period ended, mark as active
                        *status = QuarantineStatus::Active;

                        // Reset component state
                        match component {
                            ComponentType::UsbEndpoint(id) => {
                                if let Some(state) = self.usb_endpoints.get_mut(id) {
                                    state.consecutive_failures = 0;
                                    state.recent_failures.clear();
                                    state.is_stalled = false;
                                    state.stall_frame_count = 0;
                                }
                            }
                            ComponentType::NativePlugin(id) | ComponentType::WasmPlugin(id) => {
                                if let Some(state) = self.plugins.get_mut(id) {
                                    state.consecutive_overruns = 0;
                                    // Keep execution history for monitoring
                                }
                            }
                            _ => {}
                        }

                        info!(
                            "Component {} recovered successfully",
                            component.display_name()
                        );
                        return true;
                    } else {
                        debug!(
                            "Component {} still in recovery period",
                            component.display_name()
                        );
                        return false;
                    }
                }
                QuarantineStatus::Active => {
                    debug!("Component {} is already active", component.display_name());
                    return true;
                }
            }
        }

        false
    }

    /// Record a watchdog event
    fn record_event(&mut self, event: WatchdogEvent) {
        self.event_history.push_back(event);

        // Keep history bounded
        if self.event_history.len() > self.max_event_history {
            self.event_history.pop_front();
        }
    }

    /// Get recent watchdog events
    pub fn get_recent_events(&self, within: Duration) -> Vec<&WatchdogEvent> {
        let cutoff = Instant::now() - within;
        self.event_history
            .iter()
            .filter(|event| event.timestamp > cutoff)
            .collect()
    }

    /// Get all watchdog events
    pub fn get_all_events(&self) -> &VecDeque<WatchdogEvent> {
        &self.event_history
    }

    /// Get plugin overrun statistics
    pub fn get_plugin_overrun_stats(&self, plugin_id: &str) -> Option<PluginOverrunStats> {
        self.plugins.get(plugin_id).map(|state| {
            let recent_executions = state.execution_times.len();
            let recent_overruns = state
                .execution_times
                .iter()
                .filter(|(_, duration)| {
                    if let Some(config) = self
                        .configs
                        .get(&ComponentType::NativePlugin(plugin_id.to_string()))
                        .or_else(|| {
                            self.configs
                                .get(&ComponentType::WasmPlugin(plugin_id.to_string()))
                        })
                    {
                        *duration > config.max_execution_time
                    } else {
                        false
                    }
                })
                .count();

            let avg_execution_time = if !state.execution_times.is_empty() {
                let total: Duration = state.execution_times.iter().map(|(_, d)| *d).sum();
                Some(total / state.execution_times.len() as u32)
            } else {
                None
            };

            let max_execution_time = state.execution_times.iter().map(|(_, d)| *d).max();

            PluginOverrunStats {
                total_overruns: state.total_overruns,
                consecutive_overruns: state.consecutive_overruns,
                recent_executions,
                recent_overruns,
                avg_execution_time,
                max_execution_time,
                last_execution: state.last_execution,
            }
        })
    }

    /// Enable synthetic fault injection for testing
    pub fn enable_fault_injection(&mut self) {
        self.fault_injection = Some(FaultInjectionState {
            pending_faults: VecDeque::new(),
            enabled: true,
        });
        info!("Synthetic fault injection enabled");
    }

    /// Disable synthetic fault injection
    pub fn disable_fault_injection(&mut self) {
        self.fault_injection = None;
        info!("Synthetic fault injection disabled");
    }

    /// Inject a synthetic fault for testing
    pub fn inject_synthetic_fault(&mut self, fault: SyntheticFault) {
        if let Some(injection_state) = &mut self.fault_injection
            && injection_state.enabled
        {
            injection_state.pending_faults.push_back(fault);
            debug!("Synthetic fault queued for injection");
        }
    }

    /// Process pending synthetic faults
    pub fn process_synthetic_faults(&mut self) -> Vec<WatchdogEvent> {
        let mut events = Vec::new();
        let mut faults_to_process = Vec::new();

        // First, collect faults that are ready to be processed
        if let Some(injection_state) = &mut self.fault_injection
            && injection_state.enabled
        {
            let now = Instant::now();

            while let Some(fault) = injection_state.pending_faults.front() {
                if fault.inject_at <= now {
                    faults_to_process.push(injection_state.pending_faults.pop_front().unwrap());
                } else {
                    break;
                }
            }
        }

        // Now process the collected faults
        for fault in faults_to_process {
            warn!(
                "Injecting synthetic fault: {:?} for {}",
                fault.fault_type,
                fault.component.display_name()
            );

            let event = WatchdogEvent {
                timestamp: Instant::now(),
                component: fault.component.clone(),
                event_type: WatchdogEventType::SyntheticFault,
                context: fault.context,
                execution_time: None,
                action_taken: WatchdogAction::LogOnly,
            };

            self.record_event(event.clone());
            events.push(event);

            // Trigger the actual fault behavior
            match fault.fault_type {
                WatchdogEventType::PluginOverrun => {
                    if let ComponentType::NativePlugin(id) | ComponentType::WasmPlugin(id) =
                        &fault.component
                    {
                        // Simulate overrun by recording excessive execution time
                        let excessive_time = Duration::from_millis(10); // Much longer than 100μs budget
                        self.record_plugin_execution(
                            id,
                            excessive_time,
                            matches!(fault.component, ComponentType::NativePlugin(_)),
                        );
                    }
                }
                WatchdogEventType::UsbTimeout => {
                    if let ComponentType::UsbEndpoint(id) = &fault.component {
                        self.record_usb_error(id, "Synthetic USB timeout");
                    }
                }
                WatchdogEventType::NanDetected => {
                    self.check_nan_guard(f32::NAN, "synthetic_nan_test", fault.component);
                }
                _ => {}
            }
        }

        events
    }

    /// Clear all watchdog state (used for testing or reset)
    pub fn clear_all_state(&mut self) {
        self.configs.clear();
        self.quarantine_status.clear();
        self.usb_endpoints.clear();
        self.plugins.clear();
        self.event_history.clear();
        self.usb_stall_counter = 0;
        self.endpoint_wedge_timer = None;

        if let Some(injection_state) = &mut self.fault_injection {
            injection_state.pending_faults.clear();
        }

        info!("Watchdog state cleared");
    }

    /// Check if system is in fault storm (too many faults recently)
    pub fn is_in_fault_storm(&self) -> bool {
        let recent_faults = self.get_recent_events(Duration::from_secs(60));
        recent_faults.len() > 10 // More than 10 faults in last minute
    }

    /// Get system health summary
    pub fn get_health_summary(&self) -> WatchdogHealthSummary {
        let total_components = self.configs.len();
        let quarantined_components = self.get_quarantined_components().len();
        let active_components = total_components - quarantined_components;

        let recent_events = self.get_recent_events(Duration::from_secs(300)); // Last 5 minutes
        let recent_overruns = recent_events
            .iter()
            .filter(|e| e.event_type == WatchdogEventType::PluginOverrun)
            .count();
        let recent_usb_errors = recent_events
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    WatchdogEventType::UsbTimeout | WatchdogEventType::UsbError
                )
            })
            .count();
        let recent_nan_detections = recent_events
            .iter()
            .filter(|e| e.event_type == WatchdogEventType::NanDetected)
            .count();

        WatchdogHealthSummary {
            total_components,
            active_components,
            quarantined_components,
            recent_overruns,
            recent_usb_errors,
            recent_nan_detections,
            fault_injection_enabled: self
                .fault_injection
                .as_ref()
                .map(|s| s.enabled)
                .unwrap_or(false),
        }
    }
}

/// Plugin overrun statistics
#[derive(Debug, Clone)]
pub struct PluginOverrunStats {
    pub total_overruns: u32,
    pub consecutive_overruns: u32,
    pub recent_executions: usize,
    pub recent_overruns: usize,
    pub avg_execution_time: Option<Duration>,
    pub max_execution_time: Option<Duration>,
    pub last_execution: Option<Instant>,
}

/// Watchdog system health summary
#[derive(Debug, Clone)]
pub struct WatchdogHealthSummary {
    pub total_components: usize,
    pub active_components: usize,
    pub quarantined_components: usize,
    pub recent_overruns: usize,
    pub recent_usb_errors: usize,
    pub recent_nan_detections: usize,
    pub fault_injection_enabled: bool,
}

impl Default for WatchdogSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod basic_tests {
    use super::*;

    #[test]
    fn test_component_registration() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::UsbEndpoint("test_endpoint".to_string());
        let config = WatchdogConfig::default();

        watchdog.register_component(component.clone(), config);

        assert!(!watchdog.is_quarantined(&component));
        assert_eq!(
            watchdog.get_quarantine_status(&component),
            Some(&QuarantineStatus::Active)
        );
    }

    #[test]
    fn test_usb_stall_detection() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "test_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());

        watchdog.register_component(component, WatchdogConfig::default());

        // First two stalls should not trigger fault
        assert!(watchdog.record_usb_stall(endpoint_id).is_none());
        assert!(watchdog.record_usb_stall(endpoint_id).is_none());

        // Third stall should trigger fault
        let event = watchdog.record_usb_stall(endpoint_id).unwrap();
        assert_eq!(event.event_type, WatchdogEventType::UsbTimeout);
        assert_eq!(event.action_taken, WatchdogAction::ResetUsbEndpoint);
    }

    #[test]
    fn test_plugin_overrun_detection() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "test_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());

        watchdog.register_component(component, WatchdogConfig::default());

        // Normal execution should not trigger overrun
        let normal_time = Duration::from_micros(50);
        assert!(
            watchdog
                .record_plugin_execution(plugin_id, normal_time, true)
                .is_none()
        );

        // Excessive execution should trigger overrun
        let excessive_time = Duration::from_millis(1);
        let event = watchdog
            .record_plugin_execution(plugin_id, excessive_time, true)
            .unwrap();
        assert_eq!(event.event_type, WatchdogEventType::PluginOverrun);
        assert_eq!(event.execution_time, Some(excessive_time));
    }

    #[test]
    fn test_nan_guard() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::AxisNode("test_axis".to_string());
        let mut config = WatchdogConfig::default();
        config.enable_nan_guards = true;

        watchdog.register_component(component.clone(), config);

        // Normal value should not trigger guard
        assert!(
            watchdog
                .check_nan_guard(1.0, "test_value", component.clone())
                .is_none()
        );

        // NaN value should trigger guard
        let event = watchdog
            .check_nan_guard(f32::NAN, "test_nan", component)
            .unwrap();
        assert_eq!(event.event_type, WatchdogEventType::NanDetected);
    }

    #[test]
    fn test_quarantine_behavior() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "test_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        let mut config = WatchdogConfig::default();
        config.max_consecutive_failures = 2; // Lower threshold for testing

        watchdog.register_component(component.clone(), config);

        // Generate enough failures to trigger quarantine
        watchdog.record_usb_error(endpoint_id, "Test error 1");
        assert!(!watchdog.is_quarantined(&component));

        watchdog.record_usb_error(endpoint_id, "Test error 2");
        assert!(watchdog.is_quarantined(&component));

        // Check quarantine status
        if let Some(QuarantineStatus::Quarantined {
            reason,
            failure_count,
            ..
        }) = watchdog.get_quarantine_status(&component)
        {
            assert!(reason.contains("USB error"));
            assert_eq!(*failure_count, 2);
        } else {
            panic!("Component should be quarantined");
        }
    }

    #[test]
    fn test_component_recovery() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::NativePlugin("test_plugin".to_string());

        watchdog.register_component(component.clone(), WatchdogConfig::default());

        // Manually quarantine component
        watchdog.quarantine_component(&component, "Test quarantine".to_string());
        assert!(watchdog.is_quarantined(&component));

        // Attempt recovery
        assert!(watchdog.attempt_recovery(&component));

        // Should be in recovery state
        if let Some(QuarantineStatus::Recovering { .. }) =
            watchdog.get_quarantine_status(&component)
        {
            // Expected
        } else {
            panic!("Component should be in recovery state");
        }
    }

    #[test]
    fn test_synthetic_fault_injection() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::NativePlugin("test_plugin".to_string());

        watchdog.register_component(component.clone(), WatchdogConfig::default());
        watchdog.enable_fault_injection();

        let fault = SyntheticFault {
            component: component.clone(),
            fault_type: WatchdogEventType::PluginOverrun,
            inject_at: Instant::now(),
            context: "Test synthetic fault".to_string(),
        };

        watchdog.inject_synthetic_fault(fault);
        let events = watchdog.process_synthetic_faults();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, WatchdogEventType::SyntheticFault);
        assert_eq!(events[0].component, component);
    }

    #[test]
    fn test_health_summary() {
        let mut watchdog = WatchdogSystem::new();

        // Register some components
        watchdog.register_component(
            ComponentType::UsbEndpoint("ep1".to_string()),
            WatchdogConfig::default(),
        );
        watchdog.register_component(
            ComponentType::NativePlugin("plugin1".to_string()),
            WatchdogConfig::default(),
        );

        // Quarantine one component
        let component = ComponentType::UsbEndpoint("ep1".to_string());
        watchdog.quarantine_component(&component, "Test".to_string());

        let summary = watchdog.get_health_summary();

        assert_eq!(summary.total_components, 2);
        assert_eq!(summary.active_components, 1);
        assert_eq!(summary.quarantined_components, 1);
        assert!(!summary.fault_injection_enabled);
    }

    #[test]
    fn test_plugin_overrun_stats() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "test_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());

        watchdog.register_component(component, WatchdogConfig::default());

        // Record some executions
        watchdog.record_plugin_execution(plugin_id, Duration::from_micros(50), true);
        watchdog.record_plugin_execution(plugin_id, Duration::from_millis(1), true); // Overrun
        watchdog.record_plugin_execution(plugin_id, Duration::from_micros(75), true);

        let stats = watchdog.get_plugin_overrun_stats(plugin_id).unwrap();

        assert_eq!(stats.total_overruns, 1);
        assert_eq!(stats.recent_executions, 3);
        assert_eq!(stats.recent_overruns, 1);
        assert!(stats.avg_execution_time.is_some());
        assert!(stats.max_execution_time.is_some());
        assert!(stats.last_execution.is_some());
    }
}
