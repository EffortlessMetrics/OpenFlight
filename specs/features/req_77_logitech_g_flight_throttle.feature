@REQ-77 @product
Feature: Logitech G Flight Throttle Quadrant HID input parsing

  @AC-77.1
  Scenario: Report shorter than 6 bytes returns TooShort error
    Given a G Flight Throttle Quadrant HID report buffer of only 5 bytes
    When parse_g_flight_throttle is called
    Then the result SHALL be Err(TooShort)

  @AC-77.1
  Scenario: Empty buffer returns TooShort error
    Given an empty byte slice
    When parse_g_flight_throttle is called
    Then the result SHALL be Err(TooShort)

  @AC-77.2
  Scenario: All three levers at raw minimum (0) normalize to 0.0
    Given a 6-byte report with left=0, center=0, right=0
    When parse_g_flight_throttle is called
    Then axes.left SHALL be less than 0.001
    And axes.center SHALL be less than 0.001
    And axes.right SHALL be less than 0.001

  @AC-77.2
  Scenario: All three levers at raw maximum (4095) normalize to 1.0
    Given a 6-byte report with left=4095, center=4095, right=4095
    When parse_g_flight_throttle is called
    Then axes.left SHALL be greater than 0.999
    And axes.center SHALL be greater than 0.999
    And axes.right SHALL be greater than 0.999

  @AC-77.2
  Scenario: Throttle levers are independently addressable
    Given a 6-byte report with left=4095, center=0, right=2048
    When parse_g_flight_throttle is called
    Then axes.left SHALL be greater than 0.999
    And axes.center SHALL be less than 0.001
    And axes.right SHALL be within 0.49..=0.51

  @AC-77.2
  Scenario: Arbitrary byte patterns always yield lever values within 0.0..=1.0
    Given any 6 or more arbitrary bytes
    When parse_g_flight_throttle is called
    Then axes.left SHALL be within 0.0..=1.0
    And axes.center SHALL be within 0.0..=1.0
    And axes.right SHALL be within 0.0..=1.0

  @AC-77.3
  Scenario: Each of the 6 buttons is independently addressable
    Given a 6-byte report with exactly one button bit set at a time
    When parse_g_flight_throttle is called for each of the 6 buttons
    Then only that button SHALL be reported as pressed and all others SHALL be false

  @AC-77.3
  Scenario: All 6 buttons simultaneously pressed are all reported
    Given a 6-byte report with all 6 button bits set (bitmask 0x3F)
    When parse_g_flight_throttle is called
    Then button(1) through button(6) SHALL all return true

  @AC-77.3
  Scenario: Button numbers outside 1-6 always return false
    Given any valid 6-byte G Flight Throttle Quadrant report
    When button(0) or button(7..=10) is queried
    Then the result SHALL always be false
