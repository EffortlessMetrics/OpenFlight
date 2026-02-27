@REQ-89
Feature: WinWing Orion2 Throttle HID Parsing Property Invariants

  Background:
    Given the flight-hotas-winwing crate and its parse_orion2_throttle_report function

  @AC-89.1
  Scenario: A report shorter than MIN_REPORT_BYTES is rejected with TooShort
    Given a HID buffer of 10 bytes (fewer than the 24-byte minimum)
    When parse_orion2_throttle_report is called
    Then the result SHALL be Err(Orion2ThrottleParseError::TooShort)
    And the error SHALL contain the expected byte count

  @AC-89.1
  Scenario: An empty buffer is rejected with TooShort
    Given an empty HID buffer
    When parse_orion2_throttle_report is called
    Then the result SHALL be Err(Orion2ThrottleParseError::TooShort) with got=0

  @AC-89.2
  Scenario: Throttle axes are unipolar [0.0, 1.0] at minimum position
    Given a valid Orion2 Throttle report with both lever raw values at 0
    When parse_orion2_throttle_report is called
    Then axes.throttle_left SHALL be less than 0.001
    And axes.throttle_right SHALL be less than 0.001
    And axes.throttle_combined SHALL be less than 0.001

  @AC-89.2
  Scenario: Throttle axes are unipolar [0.0, 1.0] at maximum position
    Given a valid Orion2 Throttle report with both lever raw values at 0xFFFF
    When parse_orion2_throttle_report is called
    Then axes.throttle_left SHALL be within 0.0001 of 1.0
    And axes.throttle_right SHALL be within 0.0001 of 1.0
    And axes.throttle_combined SHALL be within 0.0001 of 1.0

  @AC-89.2
  Scenario: Proptest — throttle and friction axes always within [0.0, 1.0] for any u16 input
    Given any raw u16 values for throttle_left, throttle_right, and friction
    When parse_orion2_throttle_report is called
    Then axes.throttle_left SHALL be within [0.0, 1.0]
    And axes.throttle_right SHALL be within [0.0, 1.0]
    And axes.throttle_combined SHALL be within [0.0, 1.0]
    And axes.friction SHALL be within [0.0, 1.0]

  @AC-89.3
  Scenario: Axis values are finite at minimum throttle position
    Given a valid Orion2 Throttle report with both lever raw values at 0
    When parse_orion2_throttle_report is called
    Then axes.throttle_left SHALL be finite
    And axes.throttle_right SHALL be finite
    And axes.throttle_combined SHALL be finite
