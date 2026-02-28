@REQ-1023
Feature: FFB Position-Dependent Effects
  @AC-1023.1
  Scenario: FFB effect magnitude can vary based on current stick position
    Given the system is configured for REQ-1023
    When the feature condition is met
    Then ffb effect magnitude can vary based on current stick position

  @AC-1023.2
  Scenario: Position-dependent mapping is defined via configurable curve
    Given the system is configured for REQ-1023
    When the feature condition is met
    Then position-dependent mapping is defined via configurable curve

  @AC-1023.3
  Scenario: Effect transitions are smooth across position boundaries
    Given the system is configured for REQ-1023
    When the feature condition is met
    Then effect transitions are smooth across position boundaries

  @AC-1023.4
  Scenario: Position dependency is evaluated per tick without allocation
    Given the system is configured for REQ-1023
    When the feature condition is met
    Then position dependency is evaluated per tick without allocation
