@REQ-353 @axis @deadzone @hysteresis
Feature: Axis dead band hysteresis
  As a user configuring axis deadzones
  I want hysteresis on deadzone transitions
  So that the output does not chatter when the axis is near the deadzone edge

  Scenario: Output stays zero until threshold plus hysteresis is exceeded  @AC-353.1
    Given an axis with deadzone 0.05 and hysteresis 0.02
    When the axis value rises from 0.0 to 0.06 (inside deadzone + hysteresis)
    Then the output SHALL remain zero
    And when the axis value rises to 0.07 (beyond deadzone + hysteresis)
    Then the output SHALL become non-zero

  Scenario: Hysteresis amount is configurable  @AC-353.2
    Given an axis profile
    When hysteresis is set to any value between 0.0 and 0.1
    Then the configuration SHALL be accepted without error

  Scenario: Zero hysteresis produces standard deadzone behavior  @AC-353.3
    Given an axis with deadzone 0.05 and hysteresis 0.0
    When the axis value crosses 0.05 from below
    Then the output SHALL immediately become non-zero

  Scenario: Property test - no oscillation around zero with hysteresis  @AC-353.4
    Given an axis with deadzone 0.05 and hysteresis greater than 0.0
    When input values oscillate rapidly across the deadzone boundary
    Then the output SHALL not alternate between zero and non-zero on each sample

  Scenario: Hysteresis state resets on calibration reset  @AC-353.5
    Given an axis is in the hysteresis hold state (inside deadzone)
    When a calibration reset is issued
    Then the hysteresis state SHALL be cleared
    And the axis SHALL behave as if freshly initialized

  Scenario: Bipolar axes apply hysteresis symmetrically  @AC-353.6
    Given a bipolar axis with deadzone 0.05 and hysteresis 0.02
    When the axis moves from +0.01 to -0.01 (crossing center through deadzone)
    Then the output SHALL remain zero through the hysteresis band on both sides
