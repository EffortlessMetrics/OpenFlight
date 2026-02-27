Feature: Bus Backpressure Metrics
  As a flight simulation enthusiast
  I want bus backpressure metrics
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Report dropped event count
    Given the system is configured for bus backpressure metrics
    When the feature is exercised
    Then bus reports the count of dropped events due to backpressure

  Scenario: Per-subscriber drop tracking
    Given the system is configured for bus backpressure metrics
    When the feature is exercised
    Then drop counts are tracked per subscriber

  Scenario: Sustained backpressure triggers degraded status
    Given the system is configured for bus backpressure metrics
    When the feature is exercised
    Then sustained backpressure triggers a degraded health status

  Scenario: Expose via telemetry
    Given the system is configured for bus backpressure metrics
    When the feature is exercised
    Then backpressure metrics are exposed via telemetry
