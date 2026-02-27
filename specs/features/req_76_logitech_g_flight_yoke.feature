@REQ-76 @product
Feature: Logitech G Flight Yoke System HID input parsing

  @AC-76.1
  Scenario: Report shorter than 8 bytes returns TooShort error
    Given a G Flight Yoke HID report buffer of only 7 bytes
    When parse_g_flight_yoke is called
    Then the result SHALL be Err(TooShort)

  @AC-76.1
  Scenario: Empty buffer returns TooShort error
    Given an empty byte slice
    When parse_g_flight_yoke is called
    Then the result SHALL be Err(TooShort)

  @AC-76.2
  Scenario: Centered yoke produces near-zero bipolar axis values
    Given an 8-byte report with X=2048, Y=2048
    When parse_g_flight_yoke is called
    Then axes.x SHALL be within ±0.01 of 0.0
    And axes.y SHALL be within ±0.01 of 0.0

  @AC-76.2
  Scenario: Full right deflection produces x near +1.0
    Given an 8-byte report with X=4095 (maximum)
    When parse_g_flight_yoke is called
    Then axes.x SHALL be greater than 0.99

  @AC-76.2
  Scenario: Full left deflection produces x near -1.0
    Given an 8-byte report with X=0 (minimum)
    When parse_g_flight_yoke is called
    Then axes.x SHALL be less than -0.99

  @AC-76.2
  Scenario: Arbitrary byte patterns always yield bipolar axes within -1.0..=1.0
    Given any 8 or more arbitrary bytes
    When parse_g_flight_yoke is called
    Then axes.x SHALL be within -1.0..=1.0
    And axes.y SHALL be within -1.0..=1.0

  @AC-76.3
  Scenario: Rz at raw maximum (255) normalizes to 1.0
    Given an 8-byte report with Rz=255
    When parse_g_flight_yoke is called
    Then axes.rz SHALL be greater than 0.999

  @AC-76.3
  Scenario: Rz at raw minimum (0) normalizes to 0.0
    Given an 8-byte report with Rz=0
    When parse_g_flight_yoke is called
    Then axes.rz SHALL be less than 0.001

  @AC-76.3
  Scenario: Slider (mixture) at raw maximum (255) normalizes to 1.0
    Given an 8-byte report with Slider=255
    When parse_g_flight_yoke is called
    Then axes.slider SHALL be greater than 0.999

  @AC-76.3
  Scenario: Slider2 (carb heat) at raw maximum (255) normalizes to 1.0
    Given an 8-byte report with Slider2=255
    When parse_g_flight_yoke is called
    Then axes.slider2 SHALL be greater than 0.999

  @AC-76.4
  Scenario: Hat nibble 0 decodes to North
    Given an 8-byte report with hat nibble = 0
    When parse_g_flight_yoke is called
    Then buttons.hat SHALL be North

  @AC-76.4
  Scenario: Hat nibble 2 decodes to East
    Given an 8-byte report with hat nibble = 2
    When parse_g_flight_yoke is called
    Then buttons.hat SHALL be East

  @AC-76.4
  Scenario: Hat nibble 4 decodes to South
    Given an 8-byte report with hat nibble = 4
    When parse_g_flight_yoke is called
    Then buttons.hat SHALL be South

  @AC-76.4
  Scenario: Hat nibble 8..15 decodes to Center
    Given an 8-byte report with hat nibble in the range 8..15
    When parse_g_flight_yoke is called
    Then buttons.hat SHALL be Center

  @AC-76.5
  Scenario: Each of the 12 buttons is independently addressable
    Given an 8-byte report with exactly one button bit set at a time
    When parse_g_flight_yoke is called for each of the 12 buttons
    Then only that button SHALL be reported as pressed and all others SHALL be false

  @AC-76.5
  Scenario: All 12 buttons simultaneously pressed are all reported
    Given an 8-byte report with all 12 button bits set (bitmask 0x0FFF)
    When parse_g_flight_yoke is called
    Then button(1) through button(12) SHALL all return true

  @AC-76.5
  Scenario: Button numbers outside 1-12 always return false
    Given any valid 8-byte G Flight Yoke report
    When button(0) or button(13..=20) is queried
    Then the result SHALL always be false
    And the upper 4 bits of the button word SHALL always be 0
