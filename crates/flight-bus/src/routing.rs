// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Allocation-free event routing and filtering for the RT bus.
//!
//! All hot-path operations (`route_event`) use pre-allocated fixed-size arrays
//! and inline storage — no `Vec`, `String`, `HashMap`, or heap allocation.
//!
//! # Capacity
//!
//! The router supports up to [`MAX_ROUTES`] concurrent routes and returns at
//! most [`MAX_MATCHES`] matched destinations per event.

use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum number of routes the router can hold.
pub const MAX_ROUTES: usize = 64;

/// Maximum number of matched destinations returned by a single `route_event`.
pub const MAX_MATCHES: usize = 16;

/// Tolerance for floating-point comparison in `changed_only` filtering.
const CHANGED_EPSILON: f64 = 1e-9;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Source type for event origin classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceType {
    Device,
    Simulator,
    Internal,
    /// Wildcard — matches any source type.
    Any,
}

/// Topic categories for event routing.
///
/// Events are classified into topics so subscribers can filter by category.
/// `Any` matches all topics (wildcard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic {
    /// Telemetry data from simulators (airspeed, altitude, attitude, etc.).
    Telemetry,
    /// Control commands (axis movements, button presses, FFB commands).
    Commands,
    /// Lifecycle events (connect, disconnect, session start/stop).
    Lifecycle,
    /// Diagnostic and health-check events.
    Diagnostics,
    /// Wildcard — matches any topic.
    Any,
}

/// Kind of event flowing through the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    AxisUpdate,
    ButtonPress,
    ButtonRelease,
    TelemetryFrame,
    SystemStatus,
}

/// Priority level for bus events.
///
/// Ordered from lowest to highest. [`EventPriority::Critical`] events bypass
/// rate limiting. [`EventPriority::Background`] and [`EventPriority::Low`]
/// events are dropped first under backpressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventPriority {
    Background = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

/// Opaque route identifier returned by [`EventRouter::register_route`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteId(u32);

impl RouteId {
    /// Returns the raw numeric id.
    #[must_use]
    pub fn raw(self) -> u32 {
        self.0
    }
}

// ---------------------------------------------------------------------------
// RoutePattern
// ---------------------------------------------------------------------------

/// Pattern used to match incoming events to routes.
///
/// All fields support wildcard semantics: [`SourceType::Any`] matches every
/// source type, [`Topic::Any`] matches every topic, and `None` for
/// `source_id` / `event_kind` matches any value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoutePattern {
    pub source_type: SourceType,
    /// Optional specific source identifier. `None` matches any source.
    pub source_id: Option<u32>,
    /// Optional event kind filter. `None` matches any kind.
    pub event_kind: Option<EventKind>,
    /// Topic filter. [`Topic::Any`] matches all topics.
    pub topic: Topic,
}

impl RoutePattern {
    /// A pattern that matches every event.
    #[must_use]
    pub fn any() -> Self {
        Self {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: None,
            topic: Topic::Any,
        }
    }

    /// A pattern that matches events for a specific topic.
    #[must_use]
    pub fn for_topic(topic: Topic) -> Self {
        Self {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: None,
            topic,
        }
    }

    /// Check whether this pattern matches the given event attributes.
    #[inline]
    fn matches(&self, source_type: SourceType, source_id: u32, kind: EventKind, topic: Topic) -> bool {
        // Source type: Any matches everything, otherwise exact match.
        if self.source_type != SourceType::Any && self.source_type != source_type {
            return false;
        }
        // Source id: None matches everything.
        if let Some(expected_id) = self.source_id
            && expected_id != source_id
        {
            return false;
        }
        // Event kind: None matches everything.
        if let Some(expected_kind) = self.event_kind
            && expected_kind != kind
        {
            return false;
        }
        // Topic: Any matches everything.
        if self.topic != Topic::Any && self.topic != topic {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// EventFilter
// ---------------------------------------------------------------------------

/// Value and rate filters applied after pattern matching.
///
/// All fields are optional — default filter passes everything.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventFilter {
    /// Minimum axis/telemetry value (inclusive). `None` = no lower bound.
    pub min_value: Option<f64>,
    /// Maximum axis/telemetry value (inclusive). `None` = no upper bound.
    pub max_value: Option<f64>,
    /// When `true`, suppress events whose value has not changed since last delivery.
    pub changed_only: bool,
    /// Minimum milliseconds between events. 0 = no debounce.
    pub debounce_ms: u32,
    /// Maximum events per second. `None` = unlimited.
    pub rate_limit_hz: Option<f32>,
}

impl EventFilter {
    /// A pass-through filter that accepts every event.
    #[must_use]
    pub fn pass_all() -> Self {
        Self {
            min_value: None,
            max_value: None,
            changed_only: false,
            debounce_ms: 0,
            rate_limit_hz: None,
        }
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::pass_all()
    }
}

// ---------------------------------------------------------------------------
// EventPayload
// ---------------------------------------------------------------------------

/// Inline event payload — no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventPayload {
    Axis { axis_id: u16, value: f64 },
    Button { button_id: u16, pressed: bool },
    Telemetry { field_id: u16, value: f64 },
    System { code: u16 },
    Empty,
}

impl EventPayload {
    /// Extract a numeric value for filtering, if the payload carries one.
    #[inline]
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        match self {
            EventPayload::Axis { value, .. } | EventPayload::Telemetry { value, .. } => {
                Some(*value)
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// BusEvent
// ---------------------------------------------------------------------------

/// Global counter for unique event IDs.
static NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(1);

/// A single event on the bus.
///
/// All fields are `Copy` — the event can be cheaply passed by value.
#[derive(Debug, Clone, Copy)]
pub struct BusEvent {
    /// Unique event identifier.
    pub id: u64,
    /// Source classification.
    pub source_type: SourceType,
    /// Numeric source identifier.
    pub source_id: u32,
    /// Kind of event.
    pub kind: EventKind,
    /// Topic category for routing.
    pub topic: Topic,
    /// Delivery priority.
    pub priority: EventPriority,
    /// Timestamp in microseconds (monotonic).
    pub timestamp_us: u64,
    /// Typed payload.
    pub payload: EventPayload,
}

impl BusEvent {
    /// Create a new event with an auto-assigned unique ID.
    #[must_use]
    pub fn new(
        source_type: SourceType,
        source_id: u32,
        kind: EventKind,
        priority: EventPriority,
        timestamp_us: u64,
        payload: EventPayload,
    ) -> Self {
        let topic = Self::infer_topic(kind);
        Self {
            id: NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed),
            source_type,
            source_id,
            kind,
            topic,
            priority,
            timestamp_us,
            payload,
        }
    }

    /// Create a new event with an explicit topic override.
    #[must_use]
    pub fn with_topic(
        source_type: SourceType,
        source_id: u32,
        kind: EventKind,
        topic: Topic,
        priority: EventPriority,
        timestamp_us: u64,
        payload: EventPayload,
    ) -> Self {
        Self {
            id: NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed),
            source_type,
            source_id,
            kind,
            topic,
            priority,
            timestamp_us,
            payload,
        }
    }

    /// Infer topic from event kind (default mapping).
    #[inline]
    fn infer_topic(kind: EventKind) -> Topic {
        match kind {
            EventKind::TelemetryFrame => Topic::Telemetry,
            EventKind::AxisUpdate | EventKind::ButtonPress | EventKind::ButtonRelease => {
                Topic::Commands
            }
            EventKind::SystemStatus => Topic::Diagnostics,
        }
    }
}

// ---------------------------------------------------------------------------
// RouteInfo (public view of a route)
// ---------------------------------------------------------------------------

/// Read-only view of an active route.
#[derive(Debug, Clone, Copy)]
pub struct RouteInfo {
    pub id: RouteId,
    pub pattern: RoutePattern,
    pub filter: EventFilter,
    pub destination: u32,
}

// ---------------------------------------------------------------------------
// RouteMatches (allocation-free result)
// ---------------------------------------------------------------------------

/// Fixed-capacity result set from [`EventRouter::route_event`].
#[derive(Debug)]
pub struct RouteMatches {
    destinations: [u32; MAX_MATCHES],
    count: usize,
}

impl RouteMatches {
    #[inline]
    fn new() -> Self {
        Self {
            destinations: [0; MAX_MATCHES],
            count: 0,
        }
    }

    #[inline]
    fn push(&mut self, dest: u32) {
        if self.count < MAX_MATCHES {
            self.destinations[self.count] = dest;
            self.count += 1;
        }
    }

    /// Number of matched destinations.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` when no destinations matched.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get the destination at `index`, or `None` if out of range.
    #[inline]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<u32> {
        if index < self.count {
            Some(self.destinations[index])
        } else {
            None
        }
    }

    /// Iterate over matched destination IDs.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.destinations[..self.count].iter().copied()
    }

    /// Check whether `dest` is in the result set.
    #[must_use]
    pub fn contains(&self, dest: u32) -> bool {
        self.destinations[..self.count].contains(&dest)
    }
}

// ---------------------------------------------------------------------------
// Internal route / state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Route {
    id: RouteId,
    pattern: RoutePattern,
    filter: EventFilter,
    destination: u32,
}

#[derive(Debug, Clone, Copy)]
struct RouteState {
    last_value: f64,
    last_event_time_us: u64,
    events_in_window: u32,
    window_start_us: u64,
    has_previous_value: bool,
}

impl RouteState {
    const EMPTY: Self = Self {
        last_value: 0.0,
        last_event_time_us: 0,
        events_in_window: 0,
        window_start_us: 0,
        has_previous_value: false,
    };
}

// ---------------------------------------------------------------------------
// Subscriber lifecycle (allocation-free)
// ---------------------------------------------------------------------------

/// Maximum number of tracked subscribers.
pub const MAX_SUBSCRIBERS: usize = 32;

/// State of a subscriber in the bus lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriberStatus {
    /// Subscriber is active and receiving events.
    Active,
    /// Subscriber has been suspended (temporarily not receiving).
    Suspended,
    /// Subscriber has been detected as crashed/unresponsive.
    Crashed,
}

/// Per-subscriber lifecycle info tracked by the router.
#[derive(Debug, Clone, Copy)]
pub struct SubscriberInfo {
    /// Destination ID (matches route destination).
    pub destination: u32,
    /// Current lifecycle status.
    pub status: SubscriberStatus,
    /// Timestamp of last successful delivery (microseconds).
    pub last_delivery_us: u64,
    /// Count of consecutive delivery failures.
    pub consecutive_failures: u16,
    /// Total events routed to this subscriber.
    pub total_routed: u64,
    /// Total events dropped for this subscriber.
    pub total_dropped: u64,
}

impl SubscriberInfo {
    const EMPTY: Self = Self {
        destination: 0,
        status: SubscriberStatus::Active,
        last_delivery_us: 0,
        consecutive_failures: 0,
        total_routed: 0,
        total_dropped: 0,
    };
}

/// Threshold of consecutive failures before marking a subscriber as crashed.
const CRASH_THRESHOLD: u16 = 100;

// ---------------------------------------------------------------------------
// EventRouter
// ---------------------------------------------------------------------------

/// Allocation-free event router.
///
/// Supports up to [`MAX_ROUTES`] concurrent routes and [`MAX_SUBSCRIBERS`]
/// tracked subscribers. Pattern matching, topic-based filtering, value
/// filtering, changed-only suppression, debounce, rate limiting, and
/// priority-aware backpressure are all performed without heap allocation.
pub struct EventRouter {
    routes: [Option<Route>; MAX_ROUTES],
    states: [RouteState; MAX_ROUTES],
    route_count: usize,
    next_id: u32,
    backpressure_percent: u8,
    subscribers: [Option<SubscriberInfo>; MAX_SUBSCRIBERS],
    subscriber_count: usize,
}

impl EventRouter {
    /// Create a new empty router.
    #[must_use]
    pub fn new() -> Self {
        Self {
            routes: [None; MAX_ROUTES],
            states: [RouteState::EMPTY; MAX_ROUTES],
            route_count: 0,
            next_id: 1,
            backpressure_percent: 0,
            subscribers: [None; MAX_SUBSCRIBERS],
            subscriber_count: 0,
        }
    }

    /// Register a route. Returns the [`RouteId`] on success, or `None` if the
    /// router is at capacity ([`MAX_ROUTES`]).
    pub fn register_route(
        &mut self,
        pattern: RoutePattern,
        filter: EventFilter,
        destination: u32,
    ) -> Option<RouteId> {
        // Find a free slot.
        let slot = self.routes.iter().position(|r| r.is_none())?;

        let id = RouteId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);

        self.routes[slot] = Some(Route {
            id,
            pattern,
            filter,
            destination,
        });
        self.states[slot] = RouteState::EMPTY;
        self.route_count += 1;

        Some(id)
    }

    /// Remove a route by its [`RouteId`]. Returns `true` if found and removed.
    pub fn remove_route(&mut self, id: RouteId) -> bool {
        for slot in &mut self.routes {
            if let Some(route) = slot
                && route.id == id
            {
                *slot = None;
                self.route_count -= 1;
                return true;
            }
        }
        false
    }

    /// Number of active routes.
    #[inline]
    #[must_use]
    pub fn route_count(&self) -> usize {
        self.route_count
    }

    /// Iterate over all active routes.
    pub fn routes(&self) -> impl Iterator<Item = RouteInfo> + '_ {
        self.routes.iter().filter_map(|slot| {
            slot.map(|r| RouteInfo {
                id: r.id,
                pattern: r.pattern,
                filter: r.filter,
                destination: r.destination,
            })
        })
    }

    /// Set the current backpressure level (0–100).
    ///
    /// Under backpressure, low-priority events are dropped:
    /// - ≥ 25 %: drop [`EventPriority::Background`]
    /// - ≥ 50 %: drop [`EventPriority::Low`]
    /// - ≥ 75 %: drop [`EventPriority::Normal`]
    /// - [`EventPriority::High`] and [`EventPriority::Critical`] are never dropped.
    pub fn set_backpressure(&mut self, percent: u8) {
        self.backpressure_percent = percent.min(100);
    }

    /// Current backpressure level (0–100).
    #[inline]
    #[must_use]
    pub fn backpressure(&self) -> u8 {
        self.backpressure_percent
    }

    // -- subscriber lifecycle -------------------------------------------------

    /// Register a subscriber for lifecycle tracking.
    ///
    /// Returns `true` if registered, `false` if at capacity.
    pub fn register_subscriber(&mut self, destination: u32) -> bool {
        // Check for existing registration.
        for sub in &self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                return true; // Already registered.
            }
        }
        let slot = self.subscribers.iter().position(|s| s.is_none());
        match slot {
            Some(i) => {
                self.subscribers[i] = Some(SubscriberInfo {
                    destination,
                    ..SubscriberInfo::EMPTY
                });
                self.subscriber_count += 1;
                true
            }
            None => false,
        }
    }

    /// Unregister a subscriber and remove all its routes.
    pub fn unregister_subscriber(&mut self, destination: u32) {
        for sub in &mut self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                *sub = None;
                self.subscriber_count -= 1;
                break;
            }
        }
        // Remove routes targeting this destination.
        for slot in &mut self.routes {
            if let Some(route) = slot
                && route.destination == destination
            {
                *slot = None;
                self.route_count -= 1;
            }
        }
    }

    /// Mark a subscriber as crashed (e.g., detected unresponsive).
    ///
    /// Crashed subscribers are skipped during routing but their routes
    /// remain registered so they can be resumed.
    pub fn mark_subscriber_crashed(&mut self, destination: u32) {
        for sub in &mut self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                s.status = SubscriberStatus::Crashed;
                break;
            }
        }
    }

    /// Resume a previously crashed or suspended subscriber.
    pub fn resume_subscriber(&mut self, destination: u32) {
        for sub in &mut self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                s.status = SubscriberStatus::Active;
                s.consecutive_failures = 0;
                break;
            }
        }
    }

    /// Suspend a subscriber (temporarily stop routing to it).
    pub fn suspend_subscriber(&mut self, destination: u32) {
        for sub in &mut self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                s.status = SubscriberStatus::Suspended;
                break;
            }
        }
    }

    /// Get the lifecycle info for a subscriber, if tracked.
    #[must_use]
    pub fn subscriber_info(&self, destination: u32) -> Option<&SubscriberInfo> {
        self.subscribers
            .iter()
            .filter_map(|s| s.as_ref())
            .find(|s| s.destination == destination)
    }

    /// Number of tracked subscribers.
    #[inline]
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscriber_count
    }

    /// Report a delivery failure for a subscriber.
    ///
    /// When consecutive failures exceed [`CRASH_THRESHOLD`], the subscriber
    /// is automatically marked as crashed.
    pub fn record_delivery_failure(&mut self, destination: u32) {
        for sub in &mut self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                s.consecutive_failures = s.consecutive_failures.saturating_add(1);
                s.total_dropped += 1;
                if s.consecutive_failures >= CRASH_THRESHOLD {
                    s.status = SubscriberStatus::Crashed;
                }
                break;
            }
        }
    }

    /// Report a successful delivery for a subscriber.
    pub fn record_delivery_success(&mut self, destination: u32, timestamp_us: u64) {
        for sub in &mut self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                s.consecutive_failures = 0;
                s.last_delivery_us = timestamp_us;
                s.total_routed += 1;
                break;
            }
        }
    }

    /// Route an event through all active routes.
    ///
    /// Returns the set of matching destination IDs. This method **does not
    /// allocate** — it writes into a fixed-capacity [`RouteMatches`] buffer.
    ///
    /// Filter state (debounce timers, last-value tracking, rate windows) is
    /// updated in place.
    pub fn route_event(&mut self, event: &BusEvent) -> RouteMatches {
        let mut matches = RouteMatches::new();

        // Backpressure check — drop low-priority events early.
        if self.should_drop_for_backpressure(event.priority) {
            return matches;
        }

        for i in 0..MAX_ROUTES {
            let route = match &self.routes[i] {
                Some(r) => *r,
                None => continue,
            };

            // 1. Pattern match.
            if !route
                .pattern
                .matches(event.source_type, event.source_id, event.kind, event.topic)
            {
                continue;
            }

            // 1b. Skip crashed/suspended subscribers.
            if self.is_subscriber_inactive(route.destination) {
                continue;
            }

            // 2. Value range filter.
            if !Self::check_value_range(&route.filter, &event.payload) {
                continue;
            }

            // 3. Changed-only filter.
            if route.filter.changed_only && !Self::check_changed(&self.states[i], &event.payload) {
                continue;
            }

            // 4. Debounce filter.
            if route.filter.debounce_ms > 0
                && !Self::check_debounce(
                    &self.states[i],
                    route.filter.debounce_ms,
                    event.timestamp_us,
                )
            {
                continue;
            }

            // 5. Rate limit (Critical events bypass this).
            if event.priority != EventPriority::Critical
                && let Some(hz) = route.filter.rate_limit_hz
                && !Self::check_rate_limit(&mut self.states[i], hz, event.timestamp_us)
            {
                continue;
            }

            // Update per-route state.
            self.states[i].last_event_time_us = event.timestamp_us;
            if let Some(v) = event.payload.value() {
                self.states[i].last_value = v;
                self.states[i].has_previous_value = true;
            }

            matches.push(route.destination);
        }

        matches
    }

    // -- private helpers (all inline, no allocation) --------------------------

    /// Check if a subscriber (by destination) is inactive (crashed or suspended).
    /// Returns `false` if the destination has no subscriber tracking entry
    /// (untracked destinations are always routed to).
    #[inline]
    fn is_subscriber_inactive(&self, destination: u32) -> bool {
        for sub in &self.subscribers {
            if let Some(s) = sub
                && s.destination == destination
            {
                return s.status != SubscriberStatus::Active;
            }
        }
        false // Untracked destinations are treated as active.
    }

    #[inline]
    fn should_drop_for_backpressure(&self, priority: EventPriority) -> bool {
        let bp = self.backpressure_percent;
        match priority {
            EventPriority::Critical | EventPriority::High => false,
            EventPriority::Normal => bp >= 75,
            EventPriority::Low => bp >= 50,
            EventPriority::Background => bp >= 25,
        }
    }

    #[inline]
    fn check_value_range(filter: &EventFilter, payload: &EventPayload) -> bool {
        if let Some(value) = payload.value() {
            if let Some(min) = filter.min_value
                && value < min
            {
                return false;
            }
            if let Some(max) = filter.max_value
                && value > max
            {
                return false;
            }
        }
        true
    }

    #[inline]
    fn check_changed(state: &RouteState, payload: &EventPayload) -> bool {
        if !state.has_previous_value {
            return true; // First event always passes.
        }
        match payload.value() {
            Some(v) => (v - state.last_value).abs() > CHANGED_EPSILON,
            None => true, // Non-value events always pass changed-only.
        }
    }

    #[inline]
    fn check_debounce(state: &RouteState, debounce_ms: u32, timestamp_us: u64) -> bool {
        if state.last_event_time_us == 0 {
            return true; // First event always passes.
        }
        let elapsed_us = timestamp_us.saturating_sub(state.last_event_time_us);
        elapsed_us >= u64::from(debounce_ms) * 1_000
    }

    #[inline]
    fn check_rate_limit(state: &mut RouteState, max_hz: f32, timestamp_us: u64) -> bool {
        let window_us: u64 = 1_000_000; // 1 second window
        if timestamp_us.saturating_sub(state.window_start_us) >= window_us {
            // Start new window.
            state.window_start_us = timestamp_us;
            state.events_in_window = 1;
            true
        } else if (state.events_in_window as f32) < max_hz {
            state.events_in_window += 1;
            true
        } else {
            false
        }
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- helpers -------------------------------------------------------------

    fn axis_event(
        source_type: SourceType,
        source_id: u32,
        value: f64,
        priority: EventPriority,
        timestamp_us: u64,
    ) -> BusEvent {
        BusEvent::new(
            source_type,
            source_id,
            EventKind::AxisUpdate,
            priority,
            timestamp_us,
            EventPayload::Axis { axis_id: 0, value },
        )
    }

    fn button_event(
        source_type: SourceType,
        source_id: u32,
        pressed: bool,
        priority: EventPriority,
        timestamp_us: u64,
    ) -> BusEvent {
        BusEvent::new(
            source_type,
            source_id,
            EventKind::ButtonPress,
            priority,
            timestamp_us,
            EventPayload::Button {
                button_id: 1,
                pressed,
            },
        )
    }

    fn telemetry_event(timestamp_us: u64) -> BusEvent {
        BusEvent::new(
            SourceType::Simulator,
            1,
            EventKind::TelemetryFrame,
            EventPriority::Normal,
            timestamp_us,
            EventPayload::Telemetry {
                field_id: 0,
                value: 42.0,
            },
        )
    }

    // -----------------------------------------------------------------------
    // Route matching — exact, wildcard, pattern
    // -----------------------------------------------------------------------

    #[test]
    fn empty_router_returns_no_matches() {
        let mut router = EventRouter::new();
        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let m = router.route_event(&event);
        assert!(m.is_empty());
    }

    #[test]
    fn exact_source_type_match() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Device,
            source_id: None,
            event_kind: None,
            topic: Topic::Any,
        };
        router.register_route(pattern, EventFilter::pass_all(), 10);

        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let m = router.route_event(&event);
        assert_eq!(m.len(), 1);
        assert!(m.contains(10));
    }

    #[test]
    fn exact_source_type_no_match() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Simulator,
            source_id: None,
            event_kind: None,
            topic: Topic::Any,
        };
        router.register_route(pattern, EventFilter::pass_all(), 10);

        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert!(router.route_event(&event).is_empty());
    }

    #[test]
    fn wildcard_source_type_matches_all() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);

        for src in [
            SourceType::Device,
            SourceType::Simulator,
            SourceType::Internal,
        ] {
            let event = axis_event(src, 1, 0.5, EventPriority::Normal, 1000);
            assert_eq!(router.route_event(&event).len(), 1);
        }
    }

    #[test]
    fn exact_source_id_match() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Any,
            source_id: Some(42),
            event_kind: None,
            topic: Topic::Any,
        };
        router.register_route(pattern, EventFilter::pass_all(), 10);

        let hit = axis_event(SourceType::Device, 42, 0.5, EventPriority::Normal, 1000);
        let miss = axis_event(SourceType::Device, 99, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&hit).len(), 1);
        assert!(router.route_event(&miss).is_empty());
    }

    #[test]
    fn wildcard_source_id_matches_any() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: None,
            topic: Topic::Any,
        };
        router.register_route(pattern, EventFilter::pass_all(), 10);

        for id in [0, 1, 42, 999] {
            let event = axis_event(SourceType::Device, id, 0.5, EventPriority::Normal, 1000);
            assert_eq!(router.route_event(&event).len(), 1);
        }
    }

    #[test]
    fn exact_event_kind_match() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: Some(EventKind::ButtonPress),
            topic: Topic::Any,
        };
        router.register_route(pattern, EventFilter::pass_all(), 10);

        let btn = button_event(SourceType::Device, 1, true, EventPriority::Normal, 1000);
        let axis = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&btn).len(), 1);
        assert!(router.route_event(&axis).is_empty());
    }

    #[test]
    fn wildcard_event_kind_matches_any() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Any,
            source_id: None,
            event_kind: None,
            topic: Topic::Any,
        };
        router.register_route(pattern, EventFilter::pass_all(), 10);

        let btn = button_event(SourceType::Device, 1, true, EventPriority::Normal, 1000);
        let axis = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let telem = telemetry_event(1000);
        assert_eq!(router.route_event(&btn).len(), 1);
        assert_eq!(router.route_event(&axis).len(), 1);
        assert_eq!(router.route_event(&telem).len(), 1);
    }

    #[test]
    fn combined_pattern_match() {
        let mut router = EventRouter::new();
        let pattern = RoutePattern {
            source_type: SourceType::Device,
            source_id: Some(7),
            event_kind: Some(EventKind::AxisUpdate),
            topic: Topic::Any,
        };
        router.register_route(pattern, EventFilter::pass_all(), 10);

        // All three fields must match.
        let hit = axis_event(SourceType::Device, 7, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&hit).len(), 1);

        // Wrong source type.
        let miss1 = axis_event(SourceType::Simulator, 7, 0.5, EventPriority::Normal, 1000);
        assert!(router.route_event(&miss1).is_empty());

        // Wrong source id.
        let miss2 = axis_event(SourceType::Device, 8, 0.5, EventPriority::Normal, 1000);
        assert!(router.route_event(&miss2).is_empty());

        // Wrong event kind.
        let miss3 = button_event(SourceType::Device, 7, true, EventPriority::Normal, 1000);
        assert!(router.route_event(&miss3).is_empty());
    }

    // -----------------------------------------------------------------------
    // Filter — value range
    // -----------------------------------------------------------------------

    #[test]
    fn filter_min_value() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            min_value: Some(0.3),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let pass = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let fail = axis_event(SourceType::Device, 1, 0.1, EventPriority::Normal, 2000);
        let edge = axis_event(SourceType::Device, 1, 0.3, EventPriority::Normal, 3000);

        assert_eq!(router.route_event(&pass).len(), 1);
        assert!(router.route_event(&fail).is_empty());
        assert_eq!(router.route_event(&edge).len(), 1);
    }

    #[test]
    fn filter_max_value() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            max_value: Some(0.8),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let pass = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let fail = axis_event(SourceType::Device, 1, 0.9, EventPriority::Normal, 2000);
        let edge = axis_event(SourceType::Device, 1, 0.8, EventPriority::Normal, 3000);

        assert_eq!(router.route_event(&pass).len(), 1);
        assert!(router.route_event(&fail).is_empty());
        assert_eq!(router.route_event(&edge).len(), 1);
    }

    #[test]
    fn filter_value_range_band() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            min_value: Some(-0.5),
            max_value: Some(0.5),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let inside = axis_event(SourceType::Device, 1, 0.0, EventPriority::Normal, 1000);
        let below = axis_event(SourceType::Device, 1, -0.8, EventPriority::Normal, 2000);
        let above = axis_event(SourceType::Device, 1, 0.8, EventPriority::Normal, 3000);

        assert_eq!(router.route_event(&inside).len(), 1);
        assert!(router.route_event(&below).is_empty());
        assert!(router.route_event(&above).is_empty());
    }

    #[test]
    fn filter_value_range_ignores_non_value_payloads() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            min_value: Some(100.0),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        // Button events have no value — range filter should pass them through.
        let btn = button_event(SourceType::Device, 1, true, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&btn).len(), 1);
    }

    // -----------------------------------------------------------------------
    // Filter — changed only
    // -----------------------------------------------------------------------

    #[test]
    fn changed_only_first_event_passes() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            changed_only: true,
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let e1 = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&e1).len(), 1);
    }

    #[test]
    fn changed_only_suppresses_duplicate() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            changed_only: true,
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let e1 = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let e2 = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 2000);
        let e3 = axis_event(SourceType::Device, 1, 0.6, EventPriority::Normal, 3000);

        assert_eq!(router.route_event(&e1).len(), 1);
        assert!(router.route_event(&e2).is_empty()); // same value
        assert_eq!(router.route_event(&e3).len(), 1); // different value
    }

    #[test]
    fn changed_only_passes_non_value_events() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            changed_only: true,
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let b1 = button_event(SourceType::Device, 1, true, EventPriority::Normal, 1000);
        let b2 = button_event(SourceType::Device, 1, true, EventPriority::Normal, 2000);

        // Button events always pass changed_only since they have no numeric value.
        assert_eq!(router.route_event(&b1).len(), 1);
        assert_eq!(router.route_event(&b2).len(), 1);
    }

    // -----------------------------------------------------------------------
    // Filter — debounce
    // -----------------------------------------------------------------------

    #[test]
    fn debounce_first_event_passes() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            debounce_ms: 100,
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let e = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1_000);
        assert_eq!(router.route_event(&e).len(), 1);
    }

    #[test]
    fn debounce_suppresses_rapid_events() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            debounce_ms: 100, // 100ms = 100_000 us
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let e1 = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1_000_000);
        let e2 = axis_event(SourceType::Device, 1, 0.6, EventPriority::Normal, 1_050_000); // +50ms
        let e3 = axis_event(SourceType::Device, 1, 0.7, EventPriority::Normal, 1_200_000); // +200ms from e1

        assert_eq!(router.route_event(&e1).len(), 1);
        assert!(router.route_event(&e2).is_empty()); // too soon
        assert_eq!(router.route_event(&e3).len(), 1); // enough time passed since e1
    }

    #[test]
    fn debounce_zero_disables() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            debounce_ms: 0,
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let e1 = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let e2 = axis_event(SourceType::Device, 1, 0.6, EventPriority::Normal, 1001);

        assert_eq!(router.route_event(&e1).len(), 1);
        assert_eq!(router.route_event(&e2).len(), 1);
    }

    // -----------------------------------------------------------------------
    // Filter — rate limiting
    // -----------------------------------------------------------------------

    #[test]
    fn rate_limit_allows_within_budget() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            rate_limit_hz: Some(5.0),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        // 5 events at 5 Hz budget — all should pass within 1 second.
        for i in 0..5 {
            let t = 1_000_000 + i * 100_000; // 100ms apart, all in same 1s window
            let e = axis_event(
                SourceType::Device,
                1,
                i as f64 * 0.1,
                EventPriority::Normal,
                t,
            );
            assert_eq!(router.route_event(&e).len(), 1, "event {i} should pass");
        }
    }

    #[test]
    fn rate_limit_drops_excess() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            rate_limit_hz: Some(3.0),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let base = 1_000_000u64;
        for i in 0..3 {
            let e = axis_event(
                SourceType::Device,
                1,
                i as f64 * 0.1,
                EventPriority::Normal,
                base + i * 10_000,
            );
            assert_eq!(router.route_event(&e).len(), 1);
        }

        // 4th event in same window — should be dropped.
        let e4 = axis_event(
            SourceType::Device,
            1,
            0.9,
            EventPriority::Normal,
            base + 500_000,
        );
        assert!(router.route_event(&e4).is_empty());
    }

    #[test]
    fn rate_limit_resets_after_window() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            rate_limit_hz: Some(2.0),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let base = 1_000_000u64;
        // Fill budget in first window.
        for i in 0..2 {
            let e = axis_event(
                SourceType::Device,
                1,
                i as f64,
                EventPriority::Normal,
                base + i * 10_000,
            );
            assert_eq!(router.route_event(&e).len(), 1);
        }
        let e3 = axis_event(
            SourceType::Device,
            1,
            2.0,
            EventPriority::Normal,
            base + 500_000,
        );
        assert!(router.route_event(&e3).is_empty());

        // New window (>1s later).
        let e4 = axis_event(
            SourceType::Device,
            1,
            3.0,
            EventPriority::Normal,
            base + 1_500_000,
        );
        assert_eq!(router.route_event(&e4).len(), 1);
    }

    // -----------------------------------------------------------------------
    // Priority — Critical bypasses rate limiting
    // -----------------------------------------------------------------------

    #[test]
    fn critical_bypasses_rate_limit() {
        let mut router = EventRouter::new();
        let filter = EventFilter {
            rate_limit_hz: Some(1.0),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), filter, 10);

        let base = 1_000_000u64;
        // Use up the budget with a Normal event.
        let e1 = axis_event(SourceType::Device, 1, 0.1, EventPriority::Normal, base);
        assert_eq!(router.route_event(&e1).len(), 1);

        // Normal event should be rate-limited.
        let e2 = axis_event(
            SourceType::Device,
            1,
            0.2,
            EventPriority::Normal,
            base + 100_000,
        );
        assert!(router.route_event(&e2).is_empty());

        // Critical event should bypass rate limit.
        let e3 = axis_event(
            SourceType::Device,
            1,
            0.3,
            EventPriority::Critical,
            base + 200_000,
        );
        assert_eq!(router.route_event(&e3).len(), 1);
    }

    // -----------------------------------------------------------------------
    // Priority — ordering
    // -----------------------------------------------------------------------

    #[test]
    fn priority_ordering() {
        assert!(EventPriority::Critical > EventPriority::High);
        assert!(EventPriority::High > EventPriority::Normal);
        assert!(EventPriority::Normal > EventPriority::Low);
        assert!(EventPriority::Low > EventPriority::Background);
    }

    // -----------------------------------------------------------------------
    // Priority — backpressure
    // -----------------------------------------------------------------------

    #[test]
    fn backpressure_drops_background_first() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(30);

        let bg = axis_event(SourceType::Device, 1, 0.5, EventPriority::Background, 1000);
        let low = axis_event(SourceType::Device, 1, 0.5, EventPriority::Low, 2000);
        let normal = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 3000);
        let high = axis_event(SourceType::Device, 1, 0.5, EventPriority::High, 4000);
        let crit = axis_event(SourceType::Device, 1, 0.5, EventPriority::Critical, 5000);

        assert!(router.route_event(&bg).is_empty()); // dropped at 25%+
        assert_eq!(router.route_event(&low).len(), 1);
        assert_eq!(router.route_event(&normal).len(), 1);
        assert_eq!(router.route_event(&high).len(), 1);
        assert_eq!(router.route_event(&crit).len(), 1);
    }

    #[test]
    fn backpressure_drops_low_at_50() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(55);

        let bg = axis_event(SourceType::Device, 1, 0.5, EventPriority::Background, 1000);
        let low = axis_event(SourceType::Device, 1, 0.5, EventPriority::Low, 2000);
        let normal = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 3000);
        let high = axis_event(SourceType::Device, 1, 0.5, EventPriority::High, 4000);

        assert!(router.route_event(&bg).is_empty());
        assert!(router.route_event(&low).is_empty());
        assert_eq!(router.route_event(&normal).len(), 1);
        assert_eq!(router.route_event(&high).len(), 1);
    }

    #[test]
    fn backpressure_drops_normal_at_75() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(80);

        let normal = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let high = axis_event(SourceType::Device, 1, 0.5, EventPriority::High, 2000);
        let crit = axis_event(SourceType::Device, 1, 0.5, EventPriority::Critical, 3000);

        assert!(router.route_event(&normal).is_empty());
        assert_eq!(router.route_event(&high).len(), 1);
        assert_eq!(router.route_event(&crit).len(), 1);
    }

    #[test]
    fn backpressure_never_drops_critical() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(100);

        let crit = axis_event(SourceType::Device, 1, 0.5, EventPriority::Critical, 1000);
        assert_eq!(router.route_event(&crit).len(), 1);
    }

    #[test]
    fn backpressure_zero_passes_all() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.set_backpressure(0);

        let bg = axis_event(SourceType::Device, 1, 0.5, EventPriority::Background, 1000);
        assert_eq!(router.route_event(&bg).len(), 1);
    }

    // -----------------------------------------------------------------------
    // Route add / remove lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn register_returns_unique_ids() {
        let mut router = EventRouter::new();
        let id1 = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 10)
            .unwrap();
        let id2 = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 20)
            .unwrap();
        assert_ne!(id1, id2);
    }

    #[test]
    fn remove_route_succeeds() {
        let mut router = EventRouter::new();
        let id = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 10)
            .unwrap();
        assert_eq!(router.route_count(), 1);

        assert!(router.remove_route(id));
        assert_eq!(router.route_count(), 0);
    }

    #[test]
    fn remove_nonexistent_route_returns_false() {
        let mut router = EventRouter::new();
        assert!(!router.remove_route(RouteId(999)));
    }

    #[test]
    fn removed_route_no_longer_matches() {
        let mut router = EventRouter::new();
        let id = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 10)
            .unwrap();

        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&event).len(), 1);

        router.remove_route(id);
        assert!(router.route_event(&event).is_empty());
    }

    #[test]
    fn slot_reuse_after_remove() {
        let mut router = EventRouter::new();
        let id1 = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 10)
            .unwrap();
        router.remove_route(id1);

        // Should be able to add a new route in the freed slot.
        let id2 = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 20)
            .unwrap();
        assert_ne!(id1, id2);
        assert_eq!(router.route_count(), 1);
    }

    #[test]
    fn capacity_limit() {
        let mut router = EventRouter::new();
        for i in 0..MAX_ROUTES {
            assert!(
                router
                    .register_route(RoutePattern::any(), EventFilter::pass_all(), i as u32)
                    .is_some(),
                "route {i} should succeed"
            );
        }
        // Router is full — next registration should return None.
        assert!(
            router
                .register_route(RoutePattern::any(), EventFilter::pass_all(), 999)
                .is_none()
        );
    }

    #[test]
    fn routes_iterator() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 20);

        let infos: Vec<RouteInfo> = router.routes().collect();
        assert_eq!(infos.len(), 2);

        let dests: Vec<u32> = infos.iter().map(|r| r.destination).collect();
        assert!(dests.contains(&10));
        assert!(dests.contains(&20));
    }

    #[test]
    fn routes_iterator_after_remove() {
        let mut router = EventRouter::new();
        let id1 = router
            .register_route(RoutePattern::any(), EventFilter::pass_all(), 10)
            .unwrap();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 20);

        router.remove_route(id1);

        let infos: Vec<RouteInfo> = router.routes().collect();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].destination, 20);
    }

    // -----------------------------------------------------------------------
    // Multiple routes for same event
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_routes_all_match() {
        let mut router = EventRouter::new();
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 20);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 30);

        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let m = router.route_event(&event);
        assert_eq!(m.len(), 3);
        assert!(m.contains(10));
        assert!(m.contains(20));
        assert!(m.contains(30));
    }

    #[test]
    fn multiple_routes_partial_match() {
        let mut router = EventRouter::new();

        // Route 1: Device only.
        let p1 = RoutePattern {
            source_type: SourceType::Device,
            source_id: None,
            event_kind: None,
            topic: Topic::Any,
        };
        router.register_route(p1, EventFilter::pass_all(), 10);

        // Route 2: Simulator only.
        let p2 = RoutePattern {
            source_type: SourceType::Simulator,
            source_id: None,
            event_kind: None,
            topic: Topic::Any,
        };
        router.register_route(p2, EventFilter::pass_all(), 20);

        // Route 3: Any source.
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 30);

        let device_event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let m = router.route_event(&device_event);
        assert_eq!(m.len(), 2);
        assert!(m.contains(10));
        assert!(m.contains(30));
        assert!(!m.contains(20));
    }

    #[test]
    fn multiple_routes_independent_filters() {
        let mut router = EventRouter::new();

        // Route with range filter [0.0, 0.5].
        let f1 = EventFilter {
            max_value: Some(0.5),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), f1, 10);

        // Route with range filter [0.5, 1.0].
        let f2 = EventFilter {
            min_value: Some(0.5),
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), f2, 20);

        // Value 0.3 — only route 1.
        let e1 = axis_event(SourceType::Device, 1, 0.3, EventPriority::Normal, 1000);
        let m1 = router.route_event(&e1);
        assert_eq!(m1.len(), 1);
        assert!(m1.contains(10));

        // Value 0.5 — both routes (edge inclusive).
        let e2 = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 2000);
        let m2 = router.route_event(&e2);
        assert_eq!(m2.len(), 2);

        // Value 0.8 — only route 2.
        let e3 = axis_event(SourceType::Device, 1, 0.8, EventPriority::Normal, 3000);
        let m3 = router.route_event(&e3);
        assert_eq!(m3.len(), 1);
        assert!(m3.contains(20));
    }

    #[test]
    fn multiple_routes_independent_debounce_state() {
        let mut router = EventRouter::new();

        // Route 1: 100ms debounce.
        let f1 = EventFilter {
            debounce_ms: 100,
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), f1, 10);

        // Route 2: 50ms debounce.
        let f2 = EventFilter {
            debounce_ms: 50,
            ..EventFilter::pass_all()
        };
        router.register_route(RoutePattern::any(), f2, 20);

        let base = 1_000_000u64;
        let e1 = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, base);
        assert_eq!(router.route_event(&e1).len(), 2); // both pass

        // +60ms: route 2 passes (>50ms), route 1 fails (<100ms).
        let e2 = axis_event(
            SourceType::Device,
            1,
            0.6,
            EventPriority::Normal,
            base + 60_000,
        );
        let m2 = router.route_event(&e2);
        assert_eq!(m2.len(), 1);
        assert!(m2.contains(20));
    }

    // -----------------------------------------------------------------------
    // RouteMatches helpers
    // -----------------------------------------------------------------------

    #[test]
    fn route_matches_get_out_of_bounds() {
        let m = RouteMatches::new();
        assert!(m.get(0).is_none());
    }

    #[test]
    fn route_matches_iter() {
        let mut m = RouteMatches::new();
        m.push(1);
        m.push(2);
        m.push(3);

        let collected: Vec<u32> = m.iter().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    // -----------------------------------------------------------------------
    // BusEvent construction
    // -----------------------------------------------------------------------

    #[test]
    fn bus_event_ids_are_unique() {
        let e1 = BusEvent::new(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Normal,
            0,
            EventPayload::Empty,
        );
        let e2 = BusEvent::new(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            EventPriority::Normal,
            0,
            EventPayload::Empty,
        );
        assert_ne!(e1.id, e2.id);
    }

    // -----------------------------------------------------------------------
    // EventPayload::value
    // -----------------------------------------------------------------------

    #[test]
    fn payload_value_extraction() {
        assert_eq!(
            EventPayload::Axis {
                axis_id: 0,
                value: 1.5
            }
            .value(),
            Some(1.5)
        );
        assert_eq!(
            EventPayload::Telemetry {
                field_id: 0,
                value: 2.5
            }
            .value(),
            Some(2.5)
        );
        assert_eq!(
            EventPayload::Button {
                button_id: 0,
                pressed: true
            }
            .value(),
            None
        );
        assert_eq!(EventPayload::System { code: 0 }.value(), None);
        assert_eq!(EventPayload::Empty.value(), None);
    }

    // -----------------------------------------------------------------------
    // Default impls
    // -----------------------------------------------------------------------

    #[test]
    fn default_router_is_empty() {
        let router = EventRouter::default();
        assert_eq!(router.route_count(), 0);
        assert_eq!(router.backpressure(), 0);
    }

    #[test]
    fn default_filter_passes_all() {
        let f = EventFilter::default();
        assert!(f.min_value.is_none());
        assert!(f.max_value.is_none());
        assert!(!f.changed_only);
        assert_eq!(f.debounce_ms, 0);
        assert!(f.rate_limit_hz.is_none());
    }

    // -----------------------------------------------------------------------
    // Topic-based routing
    // -----------------------------------------------------------------------

    #[test]
    fn topic_infer_axis_is_commands() {
        let e = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert_eq!(e.topic, Topic::Commands);
    }

    #[test]
    fn topic_infer_telemetry_frame() {
        let e = telemetry_event(1000);
        assert_eq!(e.topic, Topic::Telemetry);
    }

    #[test]
    fn topic_infer_system_status_is_diagnostics() {
        let e = BusEvent::new(
            SourceType::Internal,
            0,
            EventKind::SystemStatus,
            EventPriority::Normal,
            1000,
            EventPayload::System { code: 1 },
        );
        assert_eq!(e.topic, Topic::Diagnostics);
    }

    #[test]
    fn topic_filter_commands_only() {
        let mut router = EventRouter::new();
        router.register_route(
            RoutePattern::for_topic(Topic::Commands),
            EventFilter::pass_all(),
            10,
        );

        let axis = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let telem = telemetry_event(2000);
        assert_eq!(router.route_event(&axis).len(), 1);
        assert!(router.route_event(&telem).is_empty());
    }

    #[test]
    fn topic_filter_telemetry_only() {
        let mut router = EventRouter::new();
        router.register_route(
            RoutePattern::for_topic(Topic::Telemetry),
            EventFilter::pass_all(),
            10,
        );

        let axis = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let telem = telemetry_event(2000);
        assert!(router.route_event(&axis).is_empty());
        assert_eq!(router.route_event(&telem).len(), 1);
    }

    #[test]
    fn topic_any_matches_all_topics() {
        let mut router = EventRouter::new();
        router.register_route(
            RoutePattern::for_topic(Topic::Any),
            EventFilter::pass_all(),
            10,
        );

        let axis = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let telem = telemetry_event(2000);
        let sys = BusEvent::new(
            SourceType::Internal,
            0,
            EventKind::SystemStatus,
            EventPriority::Normal,
            3000,
            EventPayload::System { code: 1 },
        );
        assert_eq!(router.route_event(&axis).len(), 1);
        assert_eq!(router.route_event(&telem).len(), 1);
        assert_eq!(router.route_event(&sys).len(), 1);
    }

    #[test]
    fn topic_explicit_override() {
        let e = BusEvent::with_topic(
            SourceType::Device,
            1,
            EventKind::AxisUpdate,
            Topic::Lifecycle,
            EventPriority::Normal,
            1000,
            EventPayload::Axis { axis_id: 0, value: 0.5 },
        );
        assert_eq!(e.topic, Topic::Lifecycle);
    }

    #[test]
    fn topic_multiple_routes_different_topics() {
        let mut router = EventRouter::new();
        router.register_route(
            RoutePattern::for_topic(Topic::Commands),
            EventFilter::pass_all(),
            10,
        );
        router.register_route(
            RoutePattern::for_topic(Topic::Telemetry),
            EventFilter::pass_all(),
            20,
        );

        let axis = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        let m = router.route_event(&axis);
        assert_eq!(m.len(), 1);
        assert!(m.contains(10));
        assert!(!m.contains(20));
    }

    #[test]
    fn for_topic_helper() {
        let p = RoutePattern::for_topic(Topic::Diagnostics);
        assert_eq!(p.source_type, SourceType::Any);
        assert!(p.source_id.is_none());
        assert!(p.event_kind.is_none());
        assert_eq!(p.topic, Topic::Diagnostics);
    }

    // -----------------------------------------------------------------------
    // Subscriber lifecycle
    // -----------------------------------------------------------------------

    #[test]
    fn register_subscriber() {
        let mut router = EventRouter::new();
        assert!(router.register_subscriber(10));
        assert_eq!(router.subscriber_count(), 1);
        let info = router.subscriber_info(10).unwrap();
        assert_eq!(info.status, SubscriberStatus::Active);
    }

    #[test]
    fn register_subscriber_idempotent() {
        let mut router = EventRouter::new();
        assert!(router.register_subscriber(10));
        assert!(router.register_subscriber(10));
        assert_eq!(router.subscriber_count(), 1);
    }

    #[test]
    fn unregister_subscriber_removes_routes() {
        let mut router = EventRouter::new();
        router.register_subscriber(10);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);
        assert_eq!(router.route_count(), 2);

        router.unregister_subscriber(10);
        assert_eq!(router.subscriber_count(), 0);
        assert_eq!(router.route_count(), 0);
    }

    #[test]
    fn crashed_subscriber_not_routed() {
        let mut router = EventRouter::new();
        router.register_subscriber(10);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);

        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&event).len(), 1);

        router.mark_subscriber_crashed(10);
        assert!(router.route_event(&event).is_empty());
    }

    #[test]
    fn suspended_subscriber_not_routed() {
        let mut router = EventRouter::new();
        router.register_subscriber(10);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);

        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&event).len(), 1);

        router.suspend_subscriber(10);
        assert!(router.route_event(&event).is_empty());
    }

    #[test]
    fn resumed_subscriber_receives_events() {
        let mut router = EventRouter::new();
        router.register_subscriber(10);
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);

        router.mark_subscriber_crashed(10);
        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert!(router.route_event(&event).is_empty());

        router.resume_subscriber(10);
        assert_eq!(router.route_event(&event).len(), 1);
    }

    #[test]
    fn auto_crash_after_consecutive_failures() {
        let mut router = EventRouter::new();
        router.register_subscriber(10);

        for _ in 0..100 {
            router.record_delivery_failure(10);
        }

        let info = router.subscriber_info(10).unwrap();
        assert_eq!(info.status, SubscriberStatus::Crashed);
        assert_eq!(info.consecutive_failures, 100);
        assert_eq!(info.total_dropped, 100);
    }

    #[test]
    fn delivery_success_resets_failures() {
        let mut router = EventRouter::new();
        router.register_subscriber(10);

        for _ in 0..50 {
            router.record_delivery_failure(10);
        }
        router.record_delivery_success(10, 1_000_000);

        let info = router.subscriber_info(10).unwrap();
        assert_eq!(info.consecutive_failures, 0);
        assert_eq!(info.total_routed, 1);
        assert_eq!(info.last_delivery_us, 1_000_000);
    }

    #[test]
    fn untracked_destination_always_routed() {
        let mut router = EventRouter::new();
        // Register route for destination 10 without registering subscriber.
        router.register_route(RoutePattern::any(), EventFilter::pass_all(), 10);

        let event = axis_event(SourceType::Device, 1, 0.5, EventPriority::Normal, 1000);
        assert_eq!(router.route_event(&event).len(), 1);
    }

    #[test]
    fn subscriber_capacity_limit() {
        let mut router = EventRouter::new();
        for i in 0..MAX_SUBSCRIBERS {
            assert!(router.register_subscriber(i as u32));
        }
        assert!(!router.register_subscriber(999));
    }
}
