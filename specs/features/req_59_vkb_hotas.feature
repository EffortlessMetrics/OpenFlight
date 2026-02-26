@REQ-59
Feature: VKB HOTAS HID parsing

  @AC-59.1
  Scenario: VKB health monitor tracks device success and failure thresholds
    Given a VKB device health monitor for any VKB HOTAS variant
    When successes are recorded the monitor SHALL report a healthy status
    And when failures exceed the threshold the device SHALL be reported as not connected
    And after a reset all counters and state SHALL be cleared
    And on first creation the health monitor SHALL indicate an immediate check is required

  @AC-59.2
  Scenario: STECS Modern Throttle report axes normalize to [0.0, 1.0]
    Given a STECS Modern Throttle HID report with all axis raw values at maximum (0xFFFF)
    When parse_stecs_mt_report is called with variant Mini or Max
    Then throttle, mini_left, mini_right, and rotary SHALL all be approximately 1.0
    And a report with all zeros SHALL produce 0.0 for all axes
    And the variant field SHALL be preserved in the returned state

  @AC-59.3
  Scenario: Arbitrary STECS Modern axis inputs always stay within bounds
    Given proptest generates random u16 values for throttle, mini_left, mini_right, and rotary
    When parse_stecs_mt_report is called
    Then each axis output SHALL be within the closed interval [0.0, 1.0]
    And random full-length byte slices SHALL never cause a panic

  @AC-59.4
  Scenario: Button detection in STECS Modern reports
    Given a STECS Modern report with specific button bits set in word0 and word1
    When parse_stecs_mt_report is called
    Then is_pressed(1) SHALL return true when bit 0 of word0 is set
    And is_pressed(32) SHALL return true when the MSB of word0 is set
    And is_pressed(33) SHALL return true when the LSB of word1 is set
    And is_pressed(64) SHALL return true when the MSB of word1 is set
    And is_pressed(0) and is_pressed(65) SHALL always return false

  @AC-59.5
  Scenario: Short reports are rejected with a structured error
    Given a STECS Modern HID buffer shorter than the 17-byte minimum
    When parse_stecs_mt_report is called
    Then a TooShort error SHALL be returned containing the actual byte count
    And an empty buffer SHALL also return TooShort
    And the STECS Space and Gladiator parsers SHALL return a ReportTooShort error for under-length buffers
