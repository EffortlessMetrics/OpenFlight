@REQ-467 @product
Feature: Axis Expo Curve Adjustment — Exponential Response Curves  @AC-467.1
  Scenario: Expo parameter is validated within -1.0 to 1.0 range
    Given an axis expo curve configuration
    When an expo value outside the range [-1.0, 1.0] is provided
    Then the configuration SHALL be rejected with a validation error  @AC-467.2
  Scenario: Positive expo increases center sensitivity
    Given a virtual axis with expo set to 0.5
    When a small input near center (0.1) is processed
    Then the output SHALL be larger than with a linear (expo=0.0) curve at the same input  @AC-467.3
  Scenario: Negative expo decreases center sensitivity
    Given a virtual axis with expo set to -0.5
    When a small input near center (0.1) is processed
    Then the output SHALL be smaller than with a linear (expo=0.0) curve at the same input  @AC-467.4
  Scenario: Expo curve is applied after deadzone and before output
    Given a virtual axis with a 5% deadzone and expo set to 0.3
    When an input of 0.03 is processed
    Then the deadzone filter SHALL eliminate the input before expo is applied
    And an input of 0.2 SHALL pass through deadzone and then have expo applied
