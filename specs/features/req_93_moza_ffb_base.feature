@REQ-93 @product
Feature: Moza AB9 / R3 FFB Base HID Parsing and Torque Control

  Background:
    Given the flight-ffb-moza crate with parse_ab9_report and TorqueCommand

  @AC-93.1
  Scenario: Short report returns TooShort error
    Given a HID input buffer shorter than AB9_REPORT_LEN (16 bytes)
    When parse_ab9_report is called
    Then the result SHALL be Err(MozaParseError::TooShort)
    And the empty slice SHALL also return Err

  @AC-93.1
  Scenario: Wrong report ID returns UnknownReportId error
    Given a 16-byte buffer with byte 0 set to 0x05 (not 0x01)
    When parse_ab9_report is called
    Then the result SHALL be Err(MozaParseError::UnknownReportId)

  @AC-93.2
  Scenario: All roll / pitch / twist axes are within bipolar range [-1.0, 1.0]
    Given a valid 16-byte AB9 report with report ID 0x01
    When parse_ab9_report is called
    Then axes.roll SHALL be within -1.0..=1.0
    And axes.pitch SHALL be within -1.0..=1.0
    And axes.twist SHALL be within -1.0..=1.0

  @AC-93.2
  Scenario: Full positive roll deflection produces axes.roll ≈ 1.0
    Given a 16-byte AB9 report with bytes 1-2 set to 0xFF 0x7F (i16 = +32767)
    When parse_ab9_report is called
    Then axes.roll SHALL be approximately 1.0 (within 1e-4)

  @AC-93.2
  Scenario: Throttle axis is within unipolar range [0.0, 1.0]
    Given a valid 16-byte AB9 report with throttle bytes at minimum and maximum
    When parse_ab9_report is called
    Then axes.throttle SHALL be within 0.0..=1.0
    And minimum throttle bytes SHALL produce axes.throttle < 0.01
    And maximum throttle bytes SHALL produce axes.throttle > 0.99

  @AC-93.3
  Scenario: Button bitmask byte is parsed correctly
    Given a 16-byte AB9 report with bytes 9-10 = [0b0000_0110, 0x00] (buttons 2 and 3)
    When parse_ab9_report is called
    Then buttons.is_pressed(1) SHALL be false
    And buttons.is_pressed(2) SHALL be true
    And buttons.is_pressed(3) SHALL be true

  @AC-93.3
  Scenario: Out-of-range button indices always return false
    Given a parsed AB9InputState with all buttons asserted (mask = 0xFFFF)
    When buttons.is_pressed is called with indices 0 and 17
    Then both calls SHALL return false

  @AC-93.4
  Scenario: Torque command serialises to a 6-byte report with correct report ID
    Given a TorqueCommand with x = 0.5 and y = -0.25
    When to_report is called
    Then report[0] SHALL equal TORQUE_REPORT_ID (0x20)
    And the report SHALL be exactly TORQUE_REPORT_LEN (6) bytes

  @AC-93.4
  Scenario: Torque values beyond [-1.0, 1.0] are clamped before serialisation
    Given a TorqueCommand with x = 2.5 and y = -3.0
    When to_report is called
    Then the x torque i16 decoded from bytes 1-2 SHALL equal 32767
    And the y torque i16 decoded from bytes 3-4 SHALL equal -32767

  @AC-93.4
  Scenario: Zero torque command produces all-zero payload bytes
    Given TorqueCommand::ZERO
    When to_report is called
    Then bytes 1-4 of the report SHALL all be 0x00
