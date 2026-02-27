@REQ-81 @product
Feature: VKB STECS interface HID input parsing property invariants

  @AC-81.1
  Scenario: STECS interface axes always within [0.0, 1.0] for any raw u16 input
    Given proptest generates random u16 values for all five STECS axes (rx, ry, x, y, z)
    When StecsInputHandler::parse_interface_report is called
    Then axes.rx, axes.ry, axes.x, axes.y, and axes.z SHALL each be within [0.0, 1.0]

  @AC-81.2
  Scenario: STECS interface axes are always finite
    Given proptest generates random u16 axis values for a STECS interface report
    When StecsInputHandler::parse_interface_report is called
    Then all five axes SHALL be finite (not NaN or Inf)

  @AC-81.3
  Scenario: STECS interface reports shorter than 4 bytes return ReportTooShort
    Given a STECS interface HID buffer of length 0, 1, 2, or 3
    When StecsInputHandler::parse_interface_report is called
    Then the result SHALL be Err(StecsParseError::ReportTooShort)

  @AC-81.4
  Scenario: STECS interface 32-bit button mask round-trips through parsing
    Given proptest generates a random u32 button mask stored in bytes 10-13 of the report
    When StecsInputHandler::parse_interface_report is called
    Then state.buttons SHALL exactly equal the input mask

  @AC-81.5
  Scenario: STECS Modern Throttle axes always within [0.0, 1.0] and finite
    Given proptest generates random u16 values for all four STECS Modern Throttle axes
    When parse_stecs_mt_report is called
    Then throttle, mini_left, mini_right, and rotary SHALL each be within [0.0, 1.0] and SHALL be finite

  @AC-81.5
  Scenario: STECS Modern Throttle short reports return TooShort error
    Given a STECS Modern Throttle HID buffer shorter than VKC_STECS_MT_MIN_REPORT_BYTES
    When parse_stecs_mt_report is called
    Then the result SHALL be Err(StecsMtParseError::TooShort)

  @AC-81.5
  Scenario: STECS Modern Throttle button words round-trip through parsing
    Given proptest generates random u32 values for word0 and word1
    When parse_stecs_mt_report is called
    Then state.buttons.word0 SHALL equal word0
    And state.buttons.word1 SHALL equal word1
