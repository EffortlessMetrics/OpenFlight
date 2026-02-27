@REQ-690
Feature: Axis Smoothing Adaptive Mode
  @AC-690.1
  Scenario: Adaptive mode increases smoothing when input is stable
    Given the system is configured for REQ-690
    When the feature condition is met
    Then adaptive mode increases smoothing when input is stable

  @AC-690.2
  Scenario: Adaptive mode reduces smoothing on rapid movement
    Given the system is configured for REQ-690
    When the feature condition is met
    Then adaptive mode reduces smoothing on rapid movement

  @AC-690.3
  Scenario: Adaptation rate is configurable in profile
    Given the system is configured for REQ-690
    When the feature condition is met
    Then adaptation rate is configurable in profile

  @AC-690.4
  Scenario: Adaptive mode is compatible with all other pipeline stages
    Given the system is configured for REQ-690
    When the feature condition is met
    Then adaptive mode is compatible with all other pipeline stages
