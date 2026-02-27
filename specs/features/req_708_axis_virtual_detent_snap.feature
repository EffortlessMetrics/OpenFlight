@REQ-708
Feature: Axis Virtual Detent Snap
  @AC-708.1
  Scenario: Axis output snaps to detent position within capture range
    Given the system is configured for REQ-708
    When the feature condition is met
    Then axis output snaps to detent position within capture range

  @AC-708.2
  Scenario: Snap releases when input moves beyond escape threshold
    Given the system is configured for REQ-708
    When the feature condition is met
    Then snap releases when input moves beyond escape threshold

  @AC-708.3
  Scenario: Capture and escape thresholds are independently configurable
    Given the system is configured for REQ-708
    When the feature condition is met
    Then capture and escape thresholds are independently configurable

  @AC-708.4
  Scenario: Snap behavior is smooth with configurable transition curve
    Given the system is configured for REQ-708
    When the feature condition is met
    Then snap behavior is smooth with configurable transition curve
