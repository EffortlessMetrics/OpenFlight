@REQ-1029
Feature: FFB Breakout Force
  @AC-1029.1
  Scenario: Center detent breakout force is simulated for FFB devices
    Given the system is configured for REQ-1029
    When the feature condition is met
    Then center detent breakout force is simulated for ffb devices

  @AC-1029.2
  Scenario: Breakout force magnitude is configurable per axis
    Given the system is configured for REQ-1029
    When the feature condition is met
    Then breakout force magnitude is configurable per axis

  @AC-1029.3
  Scenario: Force profile transitions smoothly past the breakout region
    Given the system is configured for REQ-1029
    When the feature condition is met
    Then force profile transitions smoothly past the breakout region

  @AC-1029.4
  Scenario: Breakout force integrates with trim offset position
    Given the system is configured for REQ-1029
    When the feature condition is met
    Then breakout force integrates with trim offset position
