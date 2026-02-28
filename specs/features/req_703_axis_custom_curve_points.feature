@REQ-703
Feature: Axis Custom Curve Points
  @AC-703.1
  Scenario: User can define custom curve via control point pairs
    Given the system is configured for REQ-703
    When the feature condition is met
    Then user can define custom curve via control point pairs

  @AC-703.2
  Scenario: Minimum of 2 points are required including origin and endpoint
    Given the system is configured for REQ-703
    When the feature condition is met
    Then minimum of 2 points are required including origin and endpoint

  @AC-703.3
  Scenario: Maximum of 16 control points are supported per axis
    Given the system is configured for REQ-703
    When the feature condition is met
    Then maximum of 16 control points are supported per axis

  @AC-703.4
  Scenario: Points are validated for monotonic X values
    Given the system is configured for REQ-703
    When the feature condition is met
    Then points are validated for monotonic x values
