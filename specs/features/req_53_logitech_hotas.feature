@REQ-53 @product
Feature: Logitech Extreme 3D Pro HID input parsing

  @AC-53.1
  Scenario: Report shorter than 7 bytes returns TooShort error
    Given an Extreme 3D Pro HID report buffer of only 6 bytes
    When parse_extreme_3d_pro is called
    Then the result SHALL be Err(TooShort)

  @AC-53.1
  Scenario: Empty buffer returns TooShort error
    Given an empty byte slice
    When parse_extreme_3d_pro is called
    Then the result SHALL be Err(TooShort)

  @AC-53.2
  Scenario: Centered stick produces near-zero bipolar axis values
    Given a 7-byte report with X=512, Y=512, Twist=128, Throttle=0
    When parse_extreme_3d_pro is called
    Then axes.x SHALL be within ±0.01 of 0.0
    And axes.y SHALL be within ±0.01 of 0.0
    And axes.twist SHALL be within ±0.01 of 0.0

  @AC-53.2
  Scenario: Full right deflection produces x near +1.0
    Given a 7-byte report with X=1023 (maximum)
    When parse_extreme_3d_pro is called
    Then axes.x SHALL be greater than 0.99

  @AC-53.2
  Scenario: Full left deflection produces x near -1.0
    Given a 7-byte report with X=0 (minimum)
    When parse_extreme_3d_pro is called
    Then axes.x SHALL be less than -0.99

  @AC-53.2
  Scenario: Full forward deflection produces y near -1.0
    Given a 7-byte report with Y=0 (minimum)
    When parse_extreme_3d_pro is called
    Then axes.y SHALL be less than -0.99

  @AC-53.2
  Scenario: Full right twist produces twist near +1.0
    Given a 7-byte report with Twist=255 (maximum)
    When parse_extreme_3d_pro is called
    Then axes.twist SHALL be greater than 0.99

  @AC-53.2
  Scenario: Full left twist produces twist near -1.0
    Given a 7-byte report with Twist=0 (minimum)
    When parse_extreme_3d_pro is called
    Then axes.twist SHALL be less than -0.99

  @AC-53.2
  Scenario: Arbitrary byte patterns always yield bipolar axes within -1.0..=1.0
    Given any 7 or more arbitrary bytes
    When parse_extreme_3d_pro is called
    Then axes.x SHALL be within -1.0..=1.0
    And axes.y SHALL be within -1.0..=1.0
    And axes.twist SHALL be within -1.0..=1.0

  @AC-53.3
  Scenario: Throttle at raw maximum (127) normalizes to 1.0
    Given a 7-byte report with Throttle=127
    When parse_extreme_3d_pro is called
    Then axes.throttle SHALL be greater than 0.999

  @AC-53.3
  Scenario: Throttle at raw minimum (0) normalizes to 0.0
    Given a 7-byte report with Throttle=0
    When parse_extreme_3d_pro is called
    Then axes.throttle SHALL be less than 0.001

  @AC-53.3
  Scenario: Throttle is always within unipolar range 0.0..=1.0
    Given any throttle raw value in 0..=127
    When parse_extreme_3d_pro is called
    Then axes.throttle SHALL be within 0.0..=1.0

  @AC-53.4
  Scenario: Hat nibble 0 decodes to North
    Given a 7-byte report with hat nibble = 0
    When parse_extreme_3d_pro is called
    Then buttons.hat SHALL be North

  @AC-53.4
  Scenario: Hat nibble 2 decodes to East
    Given a 7-byte report with hat nibble = 2
    When parse_extreme_3d_pro is called
    Then buttons.hat SHALL be East

  @AC-53.4
  Scenario: Hat nibble 4 decodes to South
    Given a 7-byte report with hat nibble = 4
    When parse_extreme_3d_pro is called
    Then buttons.hat SHALL be South

  @AC-53.4
  Scenario: Hat nibble 8..15 decodes to Center
    Given a 7-byte report with hat nibble in the range 8..15
    When parse_extreme_3d_pro is called
    Then buttons.hat SHALL be Center

  @AC-53.5
  Scenario: Each of the 12 buttons is independently addressable
    Given a 7-byte report with exactly one button bit set at a time
    When parse_extreme_3d_pro is called for each of the 12 buttons
    Then only that button SHALL be reported as pressed and all others SHALL be false

  @AC-53.5
  Scenario: All 12 buttons simultaneously pressed are all reported
    Given a 7-byte report with all 12 button bits set (bitmask 0x0FFF)
    When parse_extreme_3d_pro is called
    Then button(1) through button(12) SHALL all return true

  @AC-53.5
  Scenario: Button numbers outside 1-12 always return false
    Given any valid 7-byte Extreme 3D Pro report
    When button(0) or button(13..=20) is queried
    Then the result SHALL always be false
    And the upper 4 bits of the button word SHALL always be 0
