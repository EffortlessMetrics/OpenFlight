@REQ-79 @product
Feature: VIRPIL Constellation Alpha Prime HID input parsing

  @AC-79.1
  Scenario: Alpha Prime report shorter than minimum returns a parse error
    Given a VPC Constellation Alpha Prime HID report buffer shorter than VPC_ALPHA_PRIME_MIN_REPORT_BYTES
    When parse_alpha_prime_report is called
    Then the result SHALL be an error

  @AC-79.2
  Scenario: Arbitrary Alpha Prime axis raw values always stay within bounds
    Given proptest generates random u16 values for all five Constellation Alpha Prime axes
    When parse_alpha_prime_report is called
    Then each of axes.x, axes.y, axes.z, axes.sz, and axes.sl SHALL be within [0.0, 1.0]

  @AC-79.3
  Scenario: Left variant identity is preserved in the parsed result
    Given a valid Alpha Prime HID report and AlphaPrimeVariant::Left
    When parse_alpha_prime_report is called
    Then state.variant SHALL be Left

  @AC-79.3
  Scenario: Right variant identity is preserved in the parsed result
    Given a valid Alpha Prime HID report and AlphaPrimeVariant::Right
    When parse_alpha_prime_report is called
    Then state.variant SHALL be Right
