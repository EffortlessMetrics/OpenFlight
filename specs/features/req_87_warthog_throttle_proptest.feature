@REQ-87 @product
Feature: Thrustmaster Warthog throttle HID parsing property invariants

  @AC-87.1
  Scenario: Throttle left and right always within [0.0, 1.0] for any raw u16 input
    Given proptest generates random u16 values for throttle_left and throttle_right
    When parse_warthog_throttle is called
    Then state.axes.throttle_left SHALL be within [0.0, 1.0]
    And state.axes.throttle_right SHALL be within [0.0, 1.0]

  @AC-87.2
  Scenario: Throttle reports shorter than the minimum always return an error
    Given proptest generates a byte slice of length 0 through WARTHOG_THROTTLE_MIN_REPORT_BYTES-1
    When parse_warthog_throttle is called
    Then the result SHALL always be Err

  @AC-87.3
  Scenario: Throttle button bitmask decodes consistently for any btn_low/btn_mid/btn_high values
    Given proptest generates random u16 btn_low, u16 btn_mid, and u8 btn_high
    When parse_warthog_throttle is called
    Then state.buttons.button(n) for n in 1..=16 SHALL exactly match bit (n-1) of btn_low
    And state.buttons.button(n) for n in 17..=32 SHALL exactly match bit (n-17) of btn_mid
    And state.buttons.button(n) for n in 33..=40 SHALL exactly match bit (n-33) of btn_high
    And button(0) and button(41) SHALL return false

  @AC-87.4
  Scenario: Valid-length throttle reports always parse successfully without panic
    Given proptest generates a byte slice of length 18..40
    When parse_warthog_throttle is called
    Then the result SHALL be Ok

  @AC-87.4
  Scenario: Oversized throttle reports always parse successfully without panic
    Given proptest generates a byte slice of length WARTHOG_THROTTLE_MIN_REPORT_BYTES..256
    When parse_warthog_throttle is called
    Then parse_warthog_throttle SHALL not panic
