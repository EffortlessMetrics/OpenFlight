@REQ-695
Feature: Axis Inversion Toggle
  @AC-695.1
  Scenario: Axis direction can be inverted via profile setting
    Given the system is configured for REQ-695
    When the feature condition is met
    Then axis direction can be inverted via profile setting

  @AC-695.2
  Scenario: Inversion is applied after all processing stages
    Given the system is configured for REQ-695
    When the feature condition is met
    Then inversion is applied after all processing stages

  @AC-695.3
  Scenario: Inversion toggle is available per axis independently
    Given the system is configured for REQ-695
    When the feature condition is met
    Then inversion toggle is available per axis independently

  @AC-695.4
  Scenario: Toggling inversion takes effect immediately without restart
    Given the system is configured for REQ-695
    When the feature condition is met
    Then toggling inversion takes effect immediately without restart
