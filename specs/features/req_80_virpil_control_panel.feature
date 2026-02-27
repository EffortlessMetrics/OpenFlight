@REQ-80 @product
Feature: VIRPIL VPC Control Panel HID input parsing

  @AC-80.1
  Scenario: Panel 1 report shorter than minimum returns a parse error
    Given a VPC Control Panel 1 HID buffer shorter than VPC_PANEL1_MIN_REPORT_BYTES
    When parse_panel1_report is called
    Then the result SHALL be an error

  @AC-80.1
  Scenario: Panel 1 report at exactly the minimum length succeeds
    Given a VPC Control Panel 1 HID buffer of exactly VPC_PANEL1_MIN_REPORT_BYTES bytes
    When parse_panel1_report is called
    Then the result SHALL be Ok

  @AC-80.2
  Scenario: Each of the 48 Panel 1 buttons reflects the raw bit mask exactly
    Given proptest generates a random 48-bit button mask
    When parse_panel1_report is called
    Then is_pressed(n) for n in 1..=48 SHALL equal (mask >> (n-1)) & 1 == 1
    And is_pressed(0) and is_pressed(49) SHALL always return false

  @AC-80.3
  Scenario: Panel 2 normalised axes are always within [0.0, 1.0] and finite
    Given proptest generates random axis raw values within [0, VIRPIL_AXIS_MAX]
    When parse_panel2_report is called
    Then a1_normalised and a2_normalised SHALL each be within [0.0, 1.0] and SHALL be finite

  @AC-80.3
  Scenario: Panel 2 raw axis values round-trip through the parsed state
    Given a VPC Control Panel 2 report with a1_raw set to VIRPIL_AXIS_MAX and a2_raw set to VIRPIL_AXIS_MAX/2
    When parse_panel2_report is called
    Then state.axes.a1_raw SHALL equal VIRPIL_AXIS_MAX
    And state.axes.a2_raw SHALL equal VIRPIL_AXIS_MAX/2

  @AC-80.4
  Scenario: Each of the 47 Panel 2 buttons reflects the raw bit mask exactly
    Given proptest generates a random 47-bit button mask
    When parse_panel2_report is called
    Then is_pressed(n) for n in 1..=47 SHALL equal (mask >> (n-1)) & 1 == 1
    And is_pressed(0) and is_pressed(48) SHALL always return false

  @AC-80.5
  Scenario: Panel 2 report shorter than minimum returns a parse error
    Given a VPC Control Panel 2 HID buffer shorter than VPC_PANEL2_MIN_REPORT_BYTES
    When parse_panel2_report is called
    Then the result SHALL be an error
