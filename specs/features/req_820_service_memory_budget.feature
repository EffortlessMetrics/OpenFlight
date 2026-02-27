Feature: Service Memory Budget
  As a flight simulation enthusiast
  I want service memory budget
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Enforce configurable memory budget
    Given the system is configured for service memory budget
    When the feature is exercised
    Then service enforces a configurable memory budget for total allocation

  Scenario: Graceful load shedding near budget limit
    Given the system is configured for service memory budget
    When the feature is exercised
    Then when budget is approached, non-critical subsystems shed load gracefully

  Scenario: Report memory usage via metrics API
    Given the system is configured for service memory budget
    When the feature is exercised
    Then memory usage is reported periodically via the metrics API

  Scenario: Trigger warning on budget violation
    Given the system is configured for service memory budget
    When the feature is exercised
    Then budget violations trigger a warning event on the bus
