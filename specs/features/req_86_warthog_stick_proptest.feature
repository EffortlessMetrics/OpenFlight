@REQ-86 @product
Feature: Thrustmaster Warthog stick HID parsing property invariants

  @AC-86.1
  Scenario: Stick X/Y/Rz axes always within [-1.0, 1.0] for any raw u16 input
    Given proptest generates random u16 values for X, Y, and Rz
    When parse_warthog_stick is called
    Then state.axes.x SHALL be within [-1.0, 1.0]
    And state.axes.y SHALL be within [-1.0, 1.0]
    And state.axes.rz SHALL be within [-1.0, 1.0]

  @AC-86.2
  Scenario: Stick reports shorter than the minimum always return an error
    Given proptest generates a byte slice of length 0 through WARTHOG_STICK_MIN_REPORT_BYTES-1
    When parse_warthog_stick is called
    Then the result SHALL always be Err

  @AC-86.3
  Scenario: Stick button bitmask decodes consistently for any u16 low and u8 high values
    Given proptest generates random u16 buttons_low and u8 buttons_high
    When parse_warthog_stick is called
    Then state.buttons.button(n) for n in 1..=16 SHALL exactly match bit (n-1) of buttons_low
    And state.buttons.button(n) for n in 17..=19 SHALL exactly match bit (n-17) of buttons_high
    And button(0) and button(20) SHALL return false

  @AC-86.4
  Scenario: Valid-length stick reports always parse successfully without panic
    Given proptest generates a byte slice of length 10..32
    When parse_warthog_stick is called
    Then the result SHALL be Ok

  @AC-86.4
  Scenario: Oversized stick reports always parse successfully without panic
    Given proptest generates a byte slice of length WARTHOG_STICK_MIN_REPORT_BYTES..256
    When parse_warthog_stick is called
    Then parse_warthog_stick SHALL not panic
