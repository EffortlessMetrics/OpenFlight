@REQ-75 @product
Feature: Logitech G940 FFB HOTAS HID input parsing

  @AC-75.1
  Scenario: Joystick report shorter than 11 bytes returns JoystickTooShort error
    Given a G940 joystick HID report buffer of only 10 bytes
    When parse_g940_joystick is called
    Then the result SHALL be Err(JoystickTooShort)

  @AC-75.1
  Scenario: Empty joystick buffer returns JoystickTooShort error
    Given an empty byte slice
    When parse_g940_joystick is called
    Then the result SHALL be Err(JoystickTooShort)

  @AC-75.2
  Scenario: Centered joystick produces near-zero bipolar axis values
    Given an 11-byte report with X=2048, Y=2048, Z=0, Rz=2048
    When parse_g940_joystick is called
    Then axes.x SHALL be within ±0.01 of 0.0
    And axes.y SHALL be within ±0.01 of 0.0
    And axes.rz SHALL be within ±0.01 of 0.0

  @AC-75.2
  Scenario: Full right deflection produces X near +1.0
    Given an 11-byte report with X=4095 (maximum)
    When parse_g940_joystick is called
    Then axes.x SHALL be greater than 0.999

  @AC-75.2
  Scenario: Full left deflection produces X near -1.0
    Given an 11-byte report with X=0 (minimum)
    When parse_g940_joystick is called
    Then axes.x SHALL be less than -0.999

  @AC-75.2
  Scenario: Arbitrary byte patterns always yield bipolar axes within -1.0..=1.0
    Given any 11 or more arbitrary bytes
    When parse_g940_joystick is called
    Then axes.x SHALL be within -1.0..=1.0
    And axes.y SHALL be within -1.0..=1.0
    And axes.rz SHALL be within -1.0..=1.0

  @AC-75.3
  Scenario: Z axis at raw minimum (0) normalizes to 0.0
    Given an 11-byte report with Z=0
    When parse_g940_joystick is called
    Then axes.z SHALL be less than 0.001

  @AC-75.3
  Scenario: Z axis at raw maximum (4095) normalizes to 1.0
    Given an 11-byte report with Z=4095
    When parse_g940_joystick is called
    Then axes.z SHALL be greater than 0.999

  @AC-75.4
  Scenario: Hat nibble 8..15 decodes to Center
    Given an 11-byte report with hat nibble = 8
    When parse_g940_joystick is called
    Then hat SHALL be Center

  @AC-75.4
  Scenario: Hat nibble 0 decodes to North
    Given an 11-byte report with hat nibble = 0
    When parse_g940_joystick is called
    Then hat SHALL be North

  @AC-75.4
  Scenario: Hat nibble 2 decodes to East
    Given an 11-byte report with hat nibble = 2
    When parse_g940_joystick is called
    Then hat SHALL be East

  @AC-75.4
  Scenario: Hat nibble 4 decodes to South
    Given an 11-byte report with hat nibble = 4
    When parse_g940_joystick is called
    Then hat SHALL be South

  @AC-75.5
  Scenario: Each of the 20 joystick buttons is independently addressable
    Given an 11-byte report with exactly one button bit set at a time
    When parse_g940_joystick is called for each of the 20 buttons
    Then only that button SHALL be reported as pressed and all others SHALL be false

  @AC-75.5
  Scenario: All 20 joystick buttons simultaneously pressed are all reported
    Given an 11-byte report with all 20 button bits set (bitmask 0x000FFFFF)
    When parse_g940_joystick is called
    Then button(1) through button(20) SHALL all return true

  @AC-75.5
  Scenario: Joystick button numbers outside 1-20 always return false
    Given any valid 11-byte G940 joystick report
    When button(0) or button(21..=30) is queried
    Then the result SHALL always be false

  @AC-75.6
  Scenario: Throttle report shorter than 5 bytes returns ThrottleTooShort error
    Given a G940 throttle HID report buffer of only 4 bytes
    When parse_g940_throttle is called
    Then the result SHALL be Err(ThrottleTooShort)

  @AC-75.6
  Scenario: Empty throttle buffer returns ThrottleTooShort error
    Given an empty byte slice
    When parse_g940_throttle is called
    Then the result SHALL be Err(ThrottleTooShort)

  @AC-75.7
  Scenario: Left throttle at raw minimum (0) normalizes to 0.0
    Given a 5-byte throttle report with left=0
    When parse_g940_throttle is called
    Then left_throttle SHALL be less than 0.001

  @AC-75.7
  Scenario: Left throttle at raw maximum (4095) normalizes to 1.0
    Given a 5-byte throttle report with left=4095
    When parse_g940_throttle is called
    Then left_throttle SHALL be greater than 0.999

  @AC-75.7
  Scenario: Right throttle at raw minimum (0) normalizes to 0.0
    Given a 5-byte throttle report with right=0
    When parse_g940_throttle is called
    Then right_throttle SHALL be less than 0.001

  @AC-75.7
  Scenario: Right throttle at raw maximum (4095) normalizes to 1.0
    Given a 5-byte throttle report with right=4095
    When parse_g940_throttle is called
    Then right_throttle SHALL be greater than 0.999

  @AC-75.8
  Scenario: Each of the 11 throttle buttons is independently addressable
    Given a 5-byte throttle report with exactly one button bit set at a time
    When parse_g940_throttle is called for each of the 11 buttons
    Then only that button SHALL be reported as pressed and all others SHALL be false

  @AC-75.8
  Scenario: All 11 throttle buttons simultaneously pressed are all reported
    Given a 5-byte throttle report with all 11 button bits set (bitmask 0x07FF)
    When parse_g940_throttle is called
    Then button(1) through button(11) SHALL all return true

  @AC-75.8
  Scenario: Throttle button numbers outside 1-11 always return false
    Given any valid 5-byte G940 throttle report
    When button(0) or button(12..=20) is queried
    Then the result SHALL always be false
