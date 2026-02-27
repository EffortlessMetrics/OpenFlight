@REQ-85 @product
Feature: Honeycomb Bravo Throttle HID parsing property invariants

  @AC-85.1
  Scenario: All throttle levers always within [0.0, 1.0001] for any raw 12-bit input
    Given proptest generates random u16 raw values (0..=4095) for throttle1 and throttle2
    When parse_bravo_report is called
    Then state.axes.throttle1 SHALL be within [0.0, 1.0001]
    And state.axes.throttle2 SHALL be within [0.0, 1.0001]

  @AC-85.2
  Scenario: Any valid Bravo Throttle report with arbitrary axes and button mask always parses
    Given proptest generates arbitrary per-lever throttle values and a 64-bit button mask
    When parse_bravo_report is called
    Then the result SHALL always be Ok
