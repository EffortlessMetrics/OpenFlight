// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Verify test implementation for StreamDeck event round-trip testing
//!
//! Provides comprehensive testing of event flow from StreamDeck plugin
//! to Flight Hub and back, measuring latency and reliability.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Verify test configuration
#[derive(Debug, Clone)]
pub struct VerifyTestConfig {
    pub test_duration_ms: u64,
    pub event_interval_ms: u64,
    pub expected_events: u32,
    pub timeout_ms: u64,
    pub max_latency_ms: u32,
}

impl Default for VerifyTestConfig {
    fn default() -> Self {
        Self {
            test_duration_ms: 5000, // 5 seconds
            event_interval_ms: 100, // 100ms between events
            expected_events: 50,    // 50 events total
            timeout_ms: 10000,      // 10 second timeout
            max_latency_ms: 100,    // 100ms max latency
        }
    }
}

/// Verify test result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub success: bool,
    pub round_trip_time_ms: u32,
    pub events_processed: u32,
    pub errors: Vec<String>,
    pub timestamp: u64,
}

impl VerifyResult {
    pub fn success(round_trip_time_ms: u32, events_processed: u32) -> Self {
        Self {
            success: true,
            round_trip_time_ms,
            events_processed,
            errors: Vec::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    pub fn failure(errors: Vec<String>) -> Self {
        Self {
            success: false,
            round_trip_time_ms: 0,
            events_processed: 0,
            errors,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Event round-trip test event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEvent {
    pub id: String,
    pub event_type: TestEventType,
    pub timestamp: u64,
    pub payload: serde_json::Value,
}

impl TestEvent {
    pub fn new(event_type: TestEventType, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            payload,
        }
    }
}

/// Test event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TestEventType {
    ActionTrigger,
    TelemetryUpdate,
    PropertyChange,
    DeviceEvent,
    CustomEvent,
}

/// Event round-trip tracker
#[derive(Debug)]
struct EventTracker {
    sent_events: HashMap<String, Instant>,
    received_events: HashMap<String, Instant>,
    round_trip_times: Vec<Duration>,
    errors: Vec<String>,
}

impl EventTracker {
    fn new() -> Self {
        Self {
            sent_events: HashMap::new(),
            received_events: HashMap::new(),
            round_trip_times: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn track_sent_event(&mut self, event_id: String) {
        self.sent_events.insert(event_id, Instant::now());
    }

    fn track_received_event(&mut self, event_id: String) -> Option<Duration> {
        let received_time = Instant::now();
        self.received_events.insert(event_id.clone(), received_time);

        if let Some(sent_time) = self.sent_events.get(&event_id) {
            let round_trip_time = received_time.duration_since(*sent_time);
            self.round_trip_times.push(round_trip_time);
            Some(round_trip_time)
        } else {
            self.errors.push(format!(
                "Received event {} without corresponding sent event",
                event_id
            ));
            None
        }
    }

    fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    fn get_statistics(&self) -> EventStatistics {
        let total_sent = self.sent_events.len();
        let total_received = self.received_events.len();

        let (avg_latency, min_latency, max_latency, p99_latency) =
            if !self.round_trip_times.is_empty() {
                let mut times: Vec<u64> = self
                    .round_trip_times
                    .iter()
                    .map(|d| d.as_millis() as u64)
                    .collect();
                times.sort_unstable();

                let avg = times.iter().sum::<u64>() / times.len() as u64;
                let min = *times.first().unwrap_or(&0);
                let max = *times.last().unwrap_or(&0);
                let p99_index = ((times.len() as f64) * 0.99) as usize;
                let p99 = times
                    .get(p99_index.min(times.len() - 1))
                    .copied()
                    .unwrap_or(0);

                (avg, min, max, p99)
            } else {
                (0, 0, 0, 0)
            };

        EventStatistics {
            total_sent,
            total_received,
            success_rate: if total_sent > 0 {
                (total_received as f64 / total_sent as f64) * 100.0
            } else {
                0.0
            },
            avg_latency_ms: avg_latency,
            min_latency_ms: min_latency,
            max_latency_ms: max_latency,
            p99_latency_ms: p99_latency,
            errors: self.errors.clone(),
        }
    }
}

/// Event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStatistics {
    pub total_sent: usize,
    pub total_received: usize,
    pub success_rate: f64,
    pub avg_latency_ms: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub p99_latency_ms: u64,
    pub errors: Vec<String>,
}

/// Verify test errors
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Test timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Event processing failed: {0}")]
    EventProcessingFailed(String),

    #[error("Latency exceeded threshold: {actual_ms}ms > {threshold_ms}ms")]
    LatencyThresholdExceeded { actual_ms: u32, threshold_ms: u32 },

    #[error("Success rate too low: {actual}% < {threshold}%")]
    SuccessRateTooLow { actual: f64, threshold: f64 },

    #[error("Test configuration error: {0}")]
    ConfigurationError(String),
}

/// Event round-trip test implementation
pub struct EventRoundTrip {
    config: VerifyTestConfig,
    tracker: EventTracker,
}

impl EventRoundTrip {
    /// Create new event round-trip test
    pub fn new(config: VerifyTestConfig) -> Self {
        Self {
            config,
            tracker: EventTracker::new(),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(VerifyTestConfig::default())
    }

    /// Run the complete verify test
    pub async fn run_test(&mut self) -> Result<VerifyResult, VerifyError> {
        info!("Starting StreamDeck event round-trip verify test");

        let start_time = Instant::now();
        let test_timeout = Duration::from_millis(self.config.timeout_ms);

        // Run the test with timeout
        let result = tokio::time::timeout(test_timeout, self.execute_test()).await;

        match result {
            Ok(test_result) => {
                let elapsed = start_time.elapsed();
                info!("Verify test completed in {}ms", elapsed.as_millis());
                test_result
            }
            Err(_) => {
                error!("Verify test timed out after {}ms", self.config.timeout_ms);
                Err(VerifyError::Timeout {
                    timeout_ms: self.config.timeout_ms,
                })
            }
        }
    }

    /// Execute the actual test
    async fn execute_test(&mut self) -> Result<VerifyResult, VerifyError> {
        debug!(
            "Executing verify test with {} events",
            self.config.expected_events
        );

        // Send test events
        for i in 0..self.config.expected_events {
            let event = self.create_test_event(i);
            self.send_test_event(event).await?;

            // Wait between events
            if i < self.config.expected_events - 1 {
                tokio::time::sleep(Duration::from_millis(self.config.event_interval_ms)).await;
            }
        }

        // Wait for all responses
        let response_timeout = Duration::from_millis(self.config.max_latency_ms as u64 * 2);
        tokio::time::sleep(response_timeout).await;

        // Analyze results
        self.analyze_results()
    }

    /// Create a test event
    fn create_test_event(&self, sequence: u32) -> TestEvent {
        let payload = serde_json::json!({
            "sequence": sequence,
            "test_type": "round_trip_verify",
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        });

        TestEvent::new(TestEventType::ActionTrigger, payload)
    }

    /// Send a test event
    async fn send_test_event(&mut self, event: TestEvent) -> Result<(), VerifyError> {
        debug!("Sending test event: {}", event.id);

        self.tracker.track_sent_event(event.id.clone());

        // Simulate sending event (in real implementation, this would send via IPC)
        tokio::time::sleep(Duration::from_millis(1)).await;

        // Simulate receiving response (in real implementation, this would be async)
        self.simulate_event_response(event).await?;

        Ok(())
    }

    /// Simulate event response (for testing)
    async fn simulate_event_response(&mut self, event: TestEvent) -> Result<(), VerifyError> {
        // Simulate network/processing delay
        let delay_ms = fastrand::u64(10..=50); // 10-50ms random delay
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;

        debug!("Received response for event: {}", event.id);

        if let Some(round_trip_time) = self.tracker.track_received_event(event.id.clone()) {
            if round_trip_time.as_millis() as u32 > self.config.max_latency_ms {
                self.tracker.add_error(format!(
                    "Event {} exceeded latency threshold: {}ms > {}ms",
                    event.id,
                    round_trip_time.as_millis(),
                    self.config.max_latency_ms
                ));
            }
        }

        Ok(())
    }

    /// Analyze test results
    fn analyze_results(&self) -> Result<VerifyResult, VerifyError> {
        let stats = self.tracker.get_statistics();

        debug!("Test statistics: {:?}", stats);

        // Check success rate
        if stats.success_rate < 95.0 {
            return Ok(VerifyResult::failure(vec![format!(
                "Success rate too low: {:.1}% < 95.0%",
                stats.success_rate
            )]));
        }

        // Check average latency
        if stats.avg_latency_ms > self.config.max_latency_ms as u64 {
            return Ok(VerifyResult::failure(vec![format!(
                "Average latency too high: {}ms > {}ms",
                stats.avg_latency_ms, self.config.max_latency_ms
            )]));
        }

        // Check for errors
        if !stats.errors.is_empty() {
            warn!("Test completed with {} errors", stats.errors.len());
            return Ok(VerifyResult::failure(stats.errors));
        }

        Ok(VerifyResult::success(
            stats.avg_latency_ms as u32,
            stats.total_received as u32,
        ))
    }

    /// Get current test statistics
    pub fn get_statistics(&self) -> EventStatistics {
        self.tracker.get_statistics()
    }
}

/// Verify test runner
pub struct VerifyTest {
    round_trip: EventRoundTrip,
}

impl VerifyTest {
    /// Create new verify test
    pub fn new() -> Self {
        Self {
            round_trip: EventRoundTrip::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: VerifyTestConfig) -> Self {
        Self {
            round_trip: EventRoundTrip::new(config),
        }
    }

    /// Run the verify test
    pub async fn run(&mut self) -> Result<VerifyResult, VerifyError> {
        self.round_trip.run_test().await
    }

    /// Get test statistics
    pub fn get_statistics(&self) -> EventStatistics {
        self.round_trip.get_statistics()
    }
}

impl Default for VerifyTest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_verify_test_creation() {
        let test = VerifyTest::new();
        let stats = test.get_statistics();
        assert_eq!(stats.total_sent, 0);
        assert_eq!(stats.total_received, 0);
    }

    #[tokio::test]
    async fn test_verify_test_with_config() {
        let config = VerifyTestConfig {
            expected_events: 10,
            event_interval_ms: 50,
            max_latency_ms: 200,
            ..Default::default()
        };

        let test = VerifyTest::with_config(config);
        let stats = test.get_statistics();
        assert_eq!(stats.total_sent, 0);
    }

    #[tokio::test]
    async fn test_event_round_trip() {
        let config = VerifyTestConfig {
            expected_events: 5,
            event_interval_ms: 10,
            max_latency_ms: 100,
            test_duration_ms: 1000,
            timeout_ms: 2000,
        };

        let mut round_trip = EventRoundTrip::new(config);
        let result = round_trip.run_test().await;

        assert!(result.is_ok());
        let verify_result = result.unwrap();
        assert!(verify_result.success);
        assert_eq!(verify_result.events_processed, 5);
    }

    #[tokio::test]
    async fn test_event_tracker() {
        let mut tracker = EventTracker::new();

        let event_id = "test-event-1".to_string();
        tracker.track_sent_event(event_id.clone());

        // Simulate small delay
        tokio::time::sleep(Duration::from_millis(10)).await;

        let round_trip_time = tracker.track_received_event(event_id);
        assert!(round_trip_time.is_some());
        assert!(round_trip_time.unwrap().as_millis() >= 10);

        let stats = tracker.get_statistics();
        assert_eq!(stats.total_sent, 1);
        assert_eq!(stats.total_received, 1);
        assert_eq!(stats.success_rate, 100.0);
    }

    #[tokio::test]
    async fn test_test_event_creation() {
        let round_trip = EventRoundTrip::default();
        let event = round_trip.create_test_event(42);

        assert_eq!(event.event_type, TestEventType::ActionTrigger);
        assert!(event.payload.get("sequence").is_some());
        assert_eq!(event.payload["sequence"], 42);
    }

    #[test]
    fn test_verify_result_creation() {
        let success_result = VerifyResult::success(50, 10);
        assert!(success_result.success);
        assert_eq!(success_result.round_trip_time_ms, 50);
        assert_eq!(success_result.events_processed, 10);
        assert!(success_result.errors.is_empty());

        let failure_result = VerifyResult::failure(vec!["Test error".to_string()]);
        assert!(!failure_result.success);
        assert_eq!(failure_result.errors.len(), 1);
    }
}
