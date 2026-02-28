Feature: Device Health Monitoring
  As a flight simulation enthusiast
  I want device health monitoring
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Periodic self-test checks device connectivity and response time
    Given the system is configured for device health monitoring
    When the feature is exercised
    Then periodic self-test checks device connectivity and response time

  Scenario: Health status is reported as healthy, degraded, or disconnected
    Given the system is configured for device health monitoring
    When the feature is exercised
    Then health status is reported as healthy, degraded, or disconnected

  Scenario: Degraded health triggers an alert event on the event bus
    Given the system is configured for device health monitoring
    When the feature is exercised
    Then degraded health triggers an alert event on the event bus

  Scenario: Health history is retained for trend analysis over time
    Given the system is configured for device health monitoring
    When the feature is exercised
    Then health history is retained for trend analysis over time
