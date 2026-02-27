@REQ-82 @product
Feature: VKB Gladiator NXT EVO HID input parsing property invariants

  @AC-82.1
  Scenario: Gladiator signed axes always within [-1.0, 1.0] for any raw u16 input
    Given proptest generates random u16 values for roll, pitch, yaw, mini_x, and mini_y
    When GladiatorInputHandler::parse_report is called
    Then roll, pitch, yaw, mini_x, and mini_y SHALL each be within [-1.0, 1.0]

  @AC-82.2
  Scenario: Gladiator throttle wheel always within [0.0, 1.0]
    Given proptest generates a random u16 raw throttle value
    When GladiatorInputHandler::parse_report is called
    Then state.axes.throttle SHALL be within [0.0, 1.0]

  @AC-82.3
  Scenario: Gladiator axes are always finite for any raw input
    Given proptest generates random u16 values for roll, pitch, yaw, and throttle
    When GladiatorInputHandler::parse_report is called
    Then all four axes SHALL be finite (not NaN or Inf)

  @AC-82.4
  Scenario: Gladiator reports shorter than 12 bytes return ReportTooShort
    Given a Gladiator NXT EVO HID buffer of length 0 through 11
    When GladiatorInputHandler::parse_report is called
    Then the result SHALL be Err(GladiatorParseError::ReportTooShort)

  @AC-82.5
  Scenario: Gladiator 64 button bits correctly reflected from btn_lo and btn_hi
    Given proptest generates random u32 values for btn_lo and btn_hi
    When GladiatorInputHandler::parse_report is called
    Then state.buttons[0..31] SHALL exactly reflect the bits of btn_lo
    And state.buttons[32..63] SHALL exactly reflect the bits of btn_hi
