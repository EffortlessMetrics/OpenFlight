@REQ-83 @product
Feature: Thrustmaster HOTAS Cougar HID input parsing

  @AC-83.1
  Scenario: Centered stick reports near-zero bipolar axes
    Given a HOTAS Cougar HID report with X=32768 and Y=32768
    When parse_cougar is called
    Then axes.x SHALL be within ±0.01 of 0.0
    And axes.y SHALL be within ±0.01 of 0.0

  @AC-83.1
  Scenario: Full right deflection produces X near +1.0
    Given a HOTAS Cougar HID report with X=65535
    When parse_cougar is called
    Then axes.x SHALL be greater than 0.99

  @AC-83.1
  Scenario: Full left deflection produces X near -1.0
    Given a HOTAS Cougar HID report with X=0
    When parse_cougar is called
    Then axes.x SHALL be less than -0.99

  @AC-83.1
  Scenario: Full back pitch produces Y near +1.0
    Given a HOTAS Cougar HID report with Y=65535
    When parse_cougar is called
    Then axes.y SHALL be greater than 0.99

  @AC-83.1
  Scenario: Full forward pitch produces Y near -1.0
    Given a HOTAS Cougar HID report with Y=0
    When parse_cougar is called
    Then axes.y SHALL be less than -0.99

  @AC-83.1
  Scenario: Arbitrary u16 stick values always yield bipolar axes within -1.0..=1.0
    Given proptest generates random u16 values for X and Y
    When parse_cougar is called
    Then axes.x SHALL be within -1.0..=1.0
    And axes.y SHALL be within -1.0..=1.0

  @AC-83.2
  Scenario: Throttle at idle (raw 0) normalizes to 0.0
    Given a HOTAS Cougar HID report with throttle=0
    When parse_cougar is called
    Then axes.throttle SHALL be less than 0.001

  @AC-83.2
  Scenario: Throttle at full (raw 65535) normalizes to 1.0
    Given a HOTAS Cougar HID report with throttle=65535
    When parse_cougar is called
    Then axes.throttle SHALL be greater than 0.999

  @AC-83.2
  Scenario: Arbitrary u16 throttle values always yield unipolar axis within 0.0..=1.0
    Given proptest generates a random u16 throttle raw value
    When parse_cougar is called
    Then axes.throttle SHALL be within 0.0..=1.0

  @AC-83.3
  Scenario: Report of 9 bytes returns TooShort error
    Given a HOTAS Cougar HID buffer of only 9 bytes
    When parse_cougar is called
    Then the result SHALL be Err(CougarParseError::TooShort)

  @AC-83.3
  Scenario: Empty buffer returns TooShort error
    Given an empty byte slice
    When parse_cougar is called
    Then the result SHALL be Err(CougarParseError::TooShort) with expected=10 and actual=0

  @AC-83.3
  Scenario: Any buffer shorter than 10 bytes always returns an error
    Given proptest generates a byte slice of length 0 through 9
    When parse_cougar is called
    Then the result SHALL always be Err

  @AC-83.4
  Scenario: TG1 trigger (button 1) is detected from bit 0
    Given a HOTAS Cougar HID report with buttons bitmask 0x0001
    When parse_cougar is called
    Then button(1) SHALL return true
    And button(2) SHALL return false

  @AC-83.4
  Scenario: TG2 trigger (button 2) is detected from bit 1
    Given a HOTAS Cougar HID report with buttons bitmask 0x0002
    When parse_cougar is called
    Then button(1) SHALL return false
    And button(2) SHALL return true

  @AC-83.4
  Scenario: All 16 buttons decode consistently with the raw bitmask
    Given a HOTAS Cougar HID report with buttons bitmask 0b1010101001010101
    When parse_cougar is called
    Then each button(1..=16) SHALL return true iff the corresponding bit is set in the mask

  @AC-83.4
  Scenario: Out-of-range button indices always return false
    Given a HOTAS Cougar HID report with all button bits set (0xFFFF)
    When button(0) or button(17) is queried
    Then the result SHALL always be false

  @AC-83.4
  Scenario: Arbitrary 16-bit bitmasks always decode consistently
    Given proptest generates a random u16 button bitmask
    When parse_cougar is called
    Then button(n) SHALL exactly match bit (n-1) of the bitmask for all n in 1..=16

  @AC-83.5
  Scenario: TMS hat nibble 15 (0x0F) decodes to Center
    Given a HOTAS Cougar HID report with hat byte 0x0F
    When parse_cougar is called
    Then tms_hat SHALL be Center

  @AC-83.5
  Scenario: TMS hat nibble 0 decodes to North
    Given a HOTAS Cougar HID report with hat byte 0x00
    When parse_cougar is called
    Then tms_hat SHALL be North

  @AC-83.5
  Scenario: TMS hat nibble 2 decodes to East
    Given a HOTAS Cougar HID report with hat byte 0x02
    When parse_cougar is called
    Then tms_hat SHALL be East

  @AC-83.5
  Scenario: TMS hat nibble 4 decodes to South
    Given a HOTAS Cougar HID report with hat byte 0x04
    When parse_cougar is called
    Then tms_hat SHALL be South

  @AC-83.5
  Scenario: TMS hat nibble 6 decodes to West
    Given a HOTAS Cougar HID report with hat byte 0x06
    When parse_cougar is called
    Then tms_hat SHALL be West
