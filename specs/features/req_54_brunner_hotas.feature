@REQ-54 @product
Feature: Brunner CLS-E Force Feedback Yoke HID input parsing

  Background:
    Given the flight-hotas-brunner crate with VID 0x25BB / PID 0x0063 (PRT.5105 Yoke)

  # ─── Report validation ────────────────────────────────────────────────────────

  @AC-54.1
  Scenario: Empty slice is rejected with TooShort error
    Given an empty HID report byte slice
    When parse_cls_e_report is called
    Then the result SHALL be Err(ClsEParseError::TooShort)

  @AC-54.1
  Scenario: Slice one byte shorter than the minimum is rejected
    Given a HID report byte slice of 8 bytes (one fewer than the 9-byte minimum)
    When parse_cls_e_report is called
    Then the result SHALL be Err(ClsEParseError::TooShort(8))

  @AC-54.1
  Scenario: Error message contains the actual byte count
    Given a HID report byte slice of 5 bytes
    When parse_cls_e_report is called and the error message is inspected
    Then the error message SHALL contain the string "5"

  # ─── Happy-path parsing ───────────────────────────────────────────────────────

  @AC-54.2
  Scenario: Exactly the minimum 9-byte report parses successfully
    Given a well-formed 9-byte CLS-E HID report with report ID 0x01
    When parse_cls_e_report is called
    Then the result SHALL be Ok

  @AC-54.2
  Scenario: Reports longer than 9 bytes parse without error
    Given a 19-byte CLS-E HID report (9 mandatory + 10 padding bytes)
    When parse_cls_e_report is called
    Then the result SHALL be Ok

  # ─── Axis normalisation ───────────────────────────────────────────────────────

  @AC-54.3
  Scenario: Zero raw axis values normalise to 0.0
    Given a CLS-E report with roll=0 and pitch=0
    When parse_cls_e_report is called
    Then axes.roll SHALL be 0.0 and axes.pitch SHALL be 0.0

  @AC-54.3
  Scenario: Maximum positive raw value normalises to approximately +1.0
    Given a CLS-E report with roll=32767 (i16::MAX) and pitch=32767
    When parse_cls_e_report is called
    Then axes.roll SHALL be within 0.0001 of +1.0
    And axes.pitch SHALL be within 0.0001 of +1.0

  @AC-54.3
  Scenario: i16::MIN raw value is clamped to -1.0
    Given a CLS-E report with roll=-32768 (i16::MIN) and pitch=-32768
    When parse_cls_e_report is called
    Then axes.roll SHALL be exactly -1.0
    And axes.pitch SHALL be exactly -1.0

  @AC-54.3
  Scenario: Proptest — axes are always within [-1.0, +1.0] for all i16 inputs
    Given any i16 value for roll and any i16 value for pitch
    When parse_cls_e_report is called
    Then axes.roll SHALL be in [-1.0, +1.0]
    And axes.pitch SHALL be in [-1.0, +1.0]

  # ─── Button extraction ────────────────────────────────────────────────────────

  @AC-54.4
  Scenario: No buttons pressed when all button bytes are zero
    Given a CLS-E report with button bytes [0x00, 0x00, 0x00, 0x00]
    When parse_cls_e_report is called
    Then buttons.pressed() SHALL return an empty list

  @AC-54.4
  Scenario: Button 1 is detected when bit 0 of byte 0 is set
    Given a CLS-E report with button bytes [0x01, 0x00, 0x00, 0x00]
    When parse_cls_e_report is called
    Then buttons.is_pressed(1) SHALL be true
    And buttons.is_pressed(2) SHALL be false

  @AC-54.4
  Scenario: Button 8 is detected when bit 7 of byte 0 is set
    Given a CLS-E report with button bytes [0x80, 0x00, 0x00, 0x00]
    When parse_cls_e_report is called
    Then buttons.is_pressed(8) SHALL be true
    And buttons.is_pressed(7) SHALL be false
    And buttons.is_pressed(9) SHALL be false

  @AC-54.4
  Scenario: Button 32 is detected when bit 7 of byte 3 is set
    Given a CLS-E report with button bytes [0x00, 0x00, 0x00, 0x80]
    When parse_cls_e_report is called
    Then buttons.is_pressed(32) SHALL be true
    And buttons.is_pressed(31) SHALL be false

  @AC-54.4
  Scenario: All 32 buttons are pressed when all button bytes are 0xFF
    Given a CLS-E report with button bytes [0xFF, 0xFF, 0xFF, 0xFF]
    When parse_cls_e_report is called
    Then buttons.pressed() SHALL return a list of exactly 32 entries covering buttons 1 through 32

  @AC-54.4
  Scenario: Out-of-range button numbers always return false
    Given a CLS-E report with all button bytes set to 0xFF
    When buttons.is_pressed is called with button numbers 0, 33, and 255
    Then all three SHALL return false

  # ─── Panic safety ─────────────────────────────────────────────────────────────

  @AC-54.5
  Scenario: Proptest — parser never panics on arbitrary valid-length input
    Given any byte sequence of length 9 to 31
    When parse_cls_e_report is called
    Then it SHALL not panic (result may be Ok or Err)
