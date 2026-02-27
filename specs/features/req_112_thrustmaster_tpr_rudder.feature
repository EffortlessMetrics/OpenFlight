@REQ-112 @product
Feature: Thrustmaster TPR Rudder Pedals device support

  @AC-112.1
  Scenario: Rudder axis is derived from left-right pedal differential at full left
    Given a Thrustmaster TPR report with left pedal fully pressed and right pedal fully released
    When the rudder axis is computed
    Then rudder SHALL be within 0.001 of -1.0

  @AC-112.1
  Scenario: Rudder axis is derived from left-right pedal differential at full right
    Given a Thrustmaster TPR report with right pedal fully pressed and left pedal fully released
    When the rudder axis is computed
    Then rudder SHALL be within 0.001 of 1.0

  @AC-112.2
  Scenario: Left toe brake axis at full depression produces 1.0
    Given a Thrustmaster TPR report with left toe brake raw at maximum
    When the report is parsed
    Then left_toe_brake SHALL be within 0.001 of 1.0

  @AC-112.2
  Scenario: Right toe brake axis at full depression produces 1.0
    Given a Thrustmaster TPR report with right toe brake raw at maximum
    When the report is parsed
    Then right_toe_brake SHALL be within 0.001 of 1.0

  @AC-112.3
  Scenario: Calibration curve adjusts rudder axis output
    Given a Thrustmaster TPR device with a non-linear calibration curve applied to the rudder axis
    When a mid-travel raw rudder value is parsed
    Then the output SHALL reflect the mapped calibration value not the linear default

  @AC-112.3
  Scenario: Deadzone at center suppresses small rudder inputs
    Given a Thrustmaster TPR device with a 5% center deadzone configured
    When a raw rudder value within the deadzone band is parsed
    Then rudder SHALL be 0.0

  @AC-112.4
  Scenario: Hardware center position maps to rudder 0.0
    Given a Thrustmaster TPR report with both pedals at their hardware center position
    When the rudder axis is computed
    Then rudder SHALL be within 0.01 of 0.0

  @AC-112.4
  Scenario: Rudder axis values are always within [-1.0, 1.0] for any raw input
    Given any raw u16 pedal position value
    When the rudder axis is computed
    Then rudder SHALL be within [-1.0, 1.0]
