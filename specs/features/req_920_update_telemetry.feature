Feature: Update Telemetry
  As a flight simulation enthusiast
  I want update telemetry
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Update success and failure events are recorded with version metadata
    Given the system is configured for update telemetry
    When the feature is exercised
    Then update success and failure events are recorded with version metadata

  Scenario: Telemetry tracks download duration, size, and bandwidth utilization
    Given the system is configured for update telemetry
    When the feature is exercised
    Then telemetry tracks download duration, size, and bandwidth utilization

  Scenario: Rollback events are tracked with failure reason classification
    Given the system is configured for update telemetry
    When the feature is exercised
    Then rollback events are tracked with failure reason classification

  Scenario: Update telemetry respects user opt-in preference for data collection
    Given the system is configured for update telemetry
    When the feature is exercised
    Then update telemetry respects user opt-in preference for data collection
