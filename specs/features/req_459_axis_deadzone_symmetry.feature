@REQ-459 @product
Feature: Axis Deadzone Symmetry — Asymmetric Deadzone Configuration Support

  @AC-459.1
  Scenario: Deadzone supports separate positive and negative widths
    Given an axis with positive deadzone 0.05 and negative deadzone 0.10
    When inputs of +0.03 and -0.07 are applied
    Then both values SHALL produce an output of 0.0 as they fall within their respective deadzones

  @AC-459.2
  Scenario: Symmetric deadzone is a special case with equal widths
    Given an axis with symmetric deadzone width 0.05
    When the deadzone configuration is inspected
    Then positive_deadzone and negative_deadzone SHALL both equal 0.05

  @AC-459.3
  Scenario: Asymmetric deadzone supports brake pedals with different travel
    Given a brake pedal axis with positive deadzone 0.0 and negative deadzone 0.08
    When a small negative input of -0.04 is applied representing resting pedal weight
    Then the output SHALL be 0.0 due to the negative deadzone absorbing the resting offset

  @AC-459.4
  Scenario: Deadzone values are validated to be in 0.0 to 1.0 range
    Given a profile specifying a deadzone value of 1.5 for an axis
    When the profile is loaded
    Then loading SHALL fail with a validation error indicating the out-of-range deadzone value
