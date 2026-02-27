@REQ-702
Feature: Axis Logarithmic Response
  @AC-702.1
  Scenario: Logarithmic curve provides fine control near full deflection
    Given the system is configured for REQ-702
    When the feature condition is met
    Then logarithmic curve provides fine control near full deflection

  @AC-702.2
  Scenario: Log base is configurable to adjust curve shape
    Given the system is configured for REQ-702
    When the feature condition is met
    Then log base is configurable to adjust curve shape

  @AC-702.3
  Scenario: Logarithmic response is normalized to unit output range
    Given the system is configured for REQ-702
    When the feature condition is met
    Then logarithmic response is normalized to unit output range

  @AC-702.4
  Scenario: Extreme parameter values are clamped to safe range
    Given the system is configured for REQ-702
    When the feature condition is met
    Then extreme parameter values are clamped to safe range
