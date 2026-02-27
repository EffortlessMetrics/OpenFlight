@REQ-84 @product
Feature: Honeycomb Alpha Yoke HID parsing property invariants

  @AC-84.1
  Scenario: Roll axis always within [-1.001, 1.001] for any raw 12-bit input
    Given proptest generates a random u16 raw roll value in 0..=4095
    When parse_alpha_report is called with that roll value
    Then state.axes.roll SHALL be within [-1.001, 1.001]

  @AC-84.2
  Scenario: Pitch axis always within [-1.001, 1.001] for any raw 12-bit input
    Given proptest generates a random u16 raw pitch value in 0..=4095
    When parse_alpha_report is called with that pitch value
    Then state.axes.pitch SHALL be within [-1.001, 1.001]

  @AC-84.3
  Scenario: Any valid Alpha Yoke report always parses without error
    Given proptest generates arbitrary roll, pitch, 64-bit button mask, and hat nibble values
    When parse_alpha_report is called
    Then the result SHALL always be Ok
