@REQ-91 @product
Feature: Logitech Extreme 3D Pro HID Parsing Property Invariants

  Background:
    Given the flight-hotas-logitech crate and its parse_extreme_3d_pro function

  @AC-91.1
  Scenario: Proptest — all axes always within their declared ranges for any 7-12 byte buffer
    Given any buffer of 7 to 12 arbitrary bytes
    When parse_extreme_3d_pro is called
    Then axes.x SHALL be within -1.0..=1.0
    And axes.y SHALL be within -1.0..=1.0
    And axes.twist SHALL be within -1.0..=1.0
    And axes.throttle SHALL be within 0.0..=1.0

  @AC-91.2
  Scenario: Proptest — all axis values are finite (never NaN or Inf) for any 7-12 byte buffer
    Given any buffer of 7 to 12 arbitrary bytes
    When parse_extreme_3d_pro is called
    Then axes.x SHALL be finite
    And axes.y SHALL be finite
    And axes.twist SHALL be finite
    And axes.throttle SHALL be finite

  @AC-91.3
  Scenario: Proptest — button bitmask never exceeds the 12-bit mask for any 7-12 byte buffer
    Given any buffer of 7 to 12 arbitrary bytes
    When parse_extreme_3d_pro is called
    Then buttons.buttons SHALL be less than or equal to 0x0FFF
    And the upper 4 bits of the button word SHALL always be 0

  @AC-91.4
  Scenario: Proptest — hat decodes to a valid Extreme3DProHat variant for every nibble value 0-15
    Given a hat nibble value in 0..=15 encoded into a 7-byte report
    When parse_extreme_3d_pro is called
    Then buttons.hat SHALL be one of Center, North, NorthEast, East, SouthEast, South, SouthWest, West, NorthWest

  @AC-91.5
  Scenario: Proptest — any buffer shorter than EXTREME_3D_PRO_MIN_REPORT_BYTES returns Err
    Given a byte buffer whose length is in 0..(EXTREME_3D_PRO_MIN_REPORT_BYTES - 1)
    When parse_extreme_3d_pro is called
    Then the result SHALL be Err
    And the result SHALL never be Ok for any such short buffer
