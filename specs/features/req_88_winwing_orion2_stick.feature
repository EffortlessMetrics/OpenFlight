@REQ-88
Feature: WinWing Orion2 Stick HID Parsing Property Invariants

  Background:
    Given the flight-hotas-winwing crate and its parse_orion2_stick_report function

  @AC-88.1
  Scenario: A report shorter than MIN_REPORT_BYTES is rejected with TooShort
    Given a HID buffer of 5 bytes (fewer than the 12-byte minimum)
    When parse_orion2_stick_report is called
    Then the result SHALL be Err(Orion2StickParseError::TooShort)
    And the error SHALL contain the expected byte count

  @AC-88.1
  Scenario: An empty buffer is rejected with TooShort
    Given an empty HID buffer
    When parse_orion2_stick_report is called
    Then the result SHALL be Err(Orion2StickParseError::TooShort) with got=0

  @AC-88.2
  Scenario: Proptest — roll axis always within [-1.001, +1.001] for any i16 input
    Given any raw i16 value for roll with pitch=0
    When parse_orion2_stick_report is called
    Then axes.roll SHALL be within [-1.001, +1.001]

  @AC-88.2
  Scenario: Proptest — pitch axis always within [-1.001, +1.001] for any i16 input
    Given any raw i16 value for pitch with roll=0
    When parse_orion2_stick_report is called
    Then axes.pitch SHALL be within [-1.001, +1.001]

  @AC-88.3
  Scenario: Axis values are finite for centered input
    Given a valid Orion2 Stick report with roll=0 and pitch=0
    When parse_orion2_stick_report is called
    Then axes.roll SHALL be finite and approximately 0.0
    And axes.pitch SHALL be finite and approximately 0.0

  @AC-88.4
  Scenario: Button bit 0 set reports button 1 pressed, button 2 not pressed
    Given a valid Orion2 Stick report with button byte set to 0b00000001
    When parse_orion2_stick_report is called
    Then buttons.is_pressed(1) SHALL be true
    And buttons.is_pressed(2) SHALL be false

  @AC-88.4
  Scenario: Out-of-range button indices always return false
    Given a valid Orion2 Stick report with all button bits set
    When buttons.is_pressed is called with indices 0 and 21
    Then both SHALL return false
