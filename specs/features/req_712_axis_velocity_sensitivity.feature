@REQ-712
Feature: Axis Velocity Sensitivity
  @AC-712.1
  Scenario: Input velocity is calculated from consecutive sample differences
    Given the system is configured for REQ-712
    When the feature condition is met
    Then input velocity is calculated from consecutive sample differences

  @AC-712.2
  Scenario: Velocity value is available as a metric for diagnostics
    Given the system is configured for REQ-712
    When the feature condition is met
    Then velocity value is available as a metric for diagnostics

  @AC-712.3
  Scenario: High velocity events can trigger configurable actions
    Given the system is configured for REQ-712
    When the feature condition is met
    Then high velocity events can trigger configurable actions

  @AC-712.4
  Scenario: Velocity calculation uses configurable sample window size
    Given the system is configured for REQ-712
    When the feature condition is met
    Then velocity calculation uses configurable sample window size
