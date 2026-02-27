@REQ-55 @product
Feature: VPforce Rhino FFB Joystick HID input parsing

  Background:
    Given the flight-hotas-vpforce crate supporting Rhino v2 (VID 0x0483 / PID 0xA1C0) and Rhino v3 (PID 0xA1C1)

  # ─── Report validation ────────────────────────────────────────────────────────

  @AC-55.1
  Scenario: Empty slice is rejected with TooShort error
    Given an empty HID report byte slice
    When parse_rhino_report is called
    Then the result SHALL be Err(RhinoParseError::TooShort)

  @AC-55.1
  Scenario: A 19-byte slice (one fewer than minimum) is rejected
    Given a HID report byte slice of 19 bytes with report ID 0x01
    When parse_rhino_report is called
    Then the result SHALL be Err(RhinoParseError::TooShort)

  @AC-55.1
  Scenario: A wrong report ID byte is rejected with UnknownReportId
    Given a 20-byte HID report with first byte 0x02
    When parse_rhino_report is called
    Then the result SHALL be Err(RhinoParseError::UnknownReportId { id: 0x02 })

  @AC-55.1
  Scenario: Error message contains the offending byte count
    Given a HID report byte slice of 5 bytes with report ID 0x01
    When parse_rhino_report is called and the error message is inspected
    Then the error message SHALL contain the string "5"

  # ─── Signed axis normalisation ───────────────────────────────────────────────

  @AC-55.2
  Scenario: Centred report gives near-zero roll and pitch; throttle at midpoint
    Given a valid 20-byte Rhino report with all axis bytes zero and hat 0xFF
    When parse_rhino_report is called
    Then axes.roll SHALL be within 0.0001 of 0.0
    And axes.pitch SHALL be within 0.0001 of 0.0
    And axes.throttle SHALL be within 0.001 of 0.5

  @AC-55.2
  Scenario: Full positive roll (i16::MAX) normalises to approximately +1.0
    Given a Rhino report with roll=32767 (i16::MAX) and all other axes zero
    When parse_rhino_report is called
    Then axes.roll SHALL be within 0.0001 of +1.0

  @AC-55.2
  Scenario: Full negative roll (i16::MIN) normalises to approximately -1.0
    Given a Rhino report with roll=-32768 (i16::MIN) and all other axes zero
    When parse_rhino_report is called
    Then axes.roll SHALL be within 0.001 of -1.0

  @AC-55.2
  Scenario: Proptest — all signed axes always within [-1.0, +1.0]
    Given any i16 values for roll, pitch, rocker, twist, ry, and z
    When parse_rhino_report is called
    Then axes.roll, axes.pitch, axes.rocker, axes.twist, and axes.ry SHALL all be in [-1.0, +1.0]

  # ─── Throttle slider remapping ────────────────────────────────────────────────

  @AC-55.3
  Scenario: Throttle at i16::MIN maps to 0.0 (minimum)
    Given a Rhino report with throttle Z = -32768 (i16::MIN)
    When parse_rhino_report is called
    Then axes.throttle SHALL be less than 0.01

  @AC-55.3
  Scenario: Throttle at i16::MAX maps to 1.0 (maximum)
    Given a Rhino report with throttle Z = 32767 (i16::MAX)
    When parse_rhino_report is called
    Then axes.throttle SHALL be greater than 0.99

  # ─── Button bitmask and POV hat ───────────────────────────────────────────────

  @AC-55.4
  Scenario: Button 1 is detected when bit 0 of the 32-bit mask is set
    Given a Rhino report with button mask 0b0000_0001 and hat 0xFF
    When parse_rhino_report is called
    Then buttons.is_pressed(1) SHALL be true
    And buttons.is_pressed(2) SHALL be false

  @AC-55.4
  Scenario: Button 32 is detected when bit 31 of the 32-bit mask is set
    Given a Rhino report with button mask (1 << 31) and hat 0xFF
    When parse_rhino_report is called
    Then buttons.is_pressed(32) SHALL be true
    And buttons.is_pressed(31) SHALL be false

  @AC-55.4
  Scenario: Button numbers 0 and 33 always return false even when all bits are set
    Given a Rhino report with button mask 0xFFFF_FFFF and hat 0xFF
    When buttons.is_pressed is called with button numbers 0 and 33
    Then both SHALL return false

  @AC-55.4
  Scenario: POV hat byte is preserved exactly in parsed output
    Given a Rhino report with hat byte 0x02 (East)
    When parse_rhino_report is called
    Then buttons.hat SHALL equal 0x02

  # ─── Panic safety ─────────────────────────────────────────────────────────────

  @AC-55.5
  Scenario: Reports longer than 20 bytes are accepted without error
    Given a 28-byte buffer (20-byte valid Rhino report with 8 padding bytes appended)
    When parse_rhino_report is called
    Then the result SHALL be Ok

  @AC-55.5
  Scenario: Proptest — parser never panics on arbitrary valid-length data
    Given any byte sequence of length 20 to 39 with first byte 0x01
    When parse_rhino_report is called
    Then it SHALL not panic (result may be Ok or Err)
