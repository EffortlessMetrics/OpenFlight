@REQ-78 @product
Feature: VIRPIL Constellation Alpha HID input parsing

  @AC-78.1
  Scenario: Alpha report shorter than minimum returns a parse error
    Given a VPC Constellation Alpha HID report buffer shorter than VPC_ALPHA_MIN_REPORT_BYTES
    When parse_alpha_report is called
    Then the result SHALL be an error

  @AC-78.1
  Scenario: Alpha report exactly one byte short returns a parse error
    Given a VPC Constellation Alpha HID buffer of VPC_ALPHA_MIN_REPORT_BYTES minus one byte
    When parse_alpha_report is called
    Then the result SHALL be Err

  @AC-78.1
  Scenario: Alpha report at exactly the minimum length succeeds
    Given a VPC Constellation Alpha HID buffer of exactly VPC_ALPHA_MIN_REPORT_BYTES bytes
    When parse_alpha_report is called
    Then the result SHALL be Ok

  @AC-78.2
  Scenario: Arbitrary Alpha axis raw values always produce finite results
    Given proptest generates random u16 values for all five Constellation Alpha axes
    When parse_alpha_report is called
    Then axes.x, axes.y, axes.z, axes.sz, and axes.sl SHALL all be finite (not NaN or Inf)

  @AC-78.3
  Scenario: Each of the 28 Alpha buttons reflects the raw bit mask exactly
    Given proptest generates a random 28-bit button mask
    When parse_alpha_report is called
    Then is_pressed(n) for n in 1..=28 SHALL equal (mask >> (n-1)) & 1 == 1
    And is_pressed(0) and is_pressed(29) SHALL always return false
