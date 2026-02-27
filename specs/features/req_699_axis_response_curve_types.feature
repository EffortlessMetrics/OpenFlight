@REQ-699
Feature: Axis Response Curve Types
  @AC-699.1
  Scenario: Multiple response curve types are supported including linear, exponential, logarithmic, and S-curve
    Given the system is configured for REQ-699
    When the feature condition is met
    Then multiple response curve types are supported including linear, exponential, logarithmic, and s-curve

  @AC-699.2
  Scenario: Curve type is selectable per axis in profile
    Given the system is configured for REQ-699
    When the feature condition is met
    Then curve type is selectable per axis in profile

  @AC-699.3
  Scenario: Curve parameters are adjustable within type constraints
    Given the system is configured for REQ-699
    When the feature condition is met
    Then curve parameters are adjustable within type constraints

  @AC-699.4
  Scenario: Curve type change takes effect at next tick boundary
    Given the system is configured for REQ-699
    When the feature condition is met
    Then curve type change takes effect at next tick boundary
