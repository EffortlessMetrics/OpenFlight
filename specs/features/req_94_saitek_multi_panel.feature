@REQ-94 @product
Feature: Saitek Pro Flight Multi Panel LCD Display, LED Mask, and Button Parsing

  Background:
    Given the flight-panels-saitek crate with LcdDisplay, MultiPanelLedMask, and parse_multi_panel_input

  @AC-94.1
  Scenario: LCD from_str encodes five characters left-to-right into 7-segment bytes
    Given LcdDisplay::from_str("12345") is constructed
    When raw bytes at positions 0-4 are queried
    Then position 0 SHALL equal encode_segment('1') = 0x06
    And position 1 SHALL equal encode_segment('2') = 0x5B
    And position 2 SHALL equal encode_segment('3') = 0x4F
    And position 3 SHALL equal encode_segment('4') = 0x66
    And position 4 SHALL equal encode_segment('5') = 0x6D

  @AC-94.1
  Scenario: LCD from_str pads short strings with blank (0x00) segments on the right
    Given LcdDisplay::from_str("42") is constructed
    When raw bytes at positions 0-4 are queried
    Then position 0 SHALL equal encode_segment('4')
    And position 1 SHALL equal encode_segment('2')
    And positions 2, 3, and 4 SHALL each equal 0x00 (blank)

  @AC-94.1
  Scenario: LCD to_hid_report produces a 12-byte report in the documented layout
    Given LcdDisplay::from_str("12345") combined with LEDs ALT and VS
    When to_hid_report is called
    Then the report SHALL be exactly MULTI_PANEL_OUTPUT_BYTES (12) bytes
    And byte 0 SHALL be 0x00 (report ID)
    And bytes 1-5 SHALL contain the five 7-segment-encoded characters
    And bytes 6-10 SHALL all be 0x00 (lower row reserved)
    And byte 11 SHALL equal the LED bitmask (ALT | VS = 0x03)

  @AC-94.2
  Scenario: LED bitmask constants are distinct powers of two covering all 8 bits
    Given the eight led_bits constants ALT, VS, IAS, HDG, CRS, AUTO_THROTTLE, FLAPS, PITCH_TRIM
    When each constant is inspected
    Then every constant SHALL be a distinct power of two
    And the bitwise OR of all constants SHALL equal 0xFF

  @AC-94.2
  Scenario: MultiPanelLedMask NONE is 0x00 and ALL is 0xFF
    Given MultiPanelLedMask::NONE and MultiPanelLedMask::ALL
    When raw() is called on each
    Then NONE.raw() SHALL equal 0x00
    And ALL.raw() SHALL equal 0xFF

  @AC-94.2
  Scenario: LED mask set builder correctly sets and clears individual bits
    Given MultiPanelLedMask::NONE with ALT and HDG set to true
    When is_set is called for ALT, HDG, and VS
    Then ALT SHALL be set
    And HDG SHALL be set
    And VS SHALL not be set
    And clearing ALT SHALL leave HDG still set

  @AC-94.3
  Scenario: Button state parsing extracts mode selector bits from byte 1
    Given a 3-byte HID input report [0x00, 0b0001_0101, 0x00]
    When parse_multi_panel_input is called
    Then sel_alt() SHALL be true (bit 0)
    And sel_vs() SHALL be false (bit 1)
    And sel_ias() SHALL be true (bit 2)
    And sel_hdg() SHALL be false (bit 3)
    And sel_crs() SHALL be true (bit 4)

  @AC-94.3
  Scenario: Button state parsing extracts all AP function buttons from byte 2
    Given a 3-byte HID input report [0x00, 0x00, 0xFF]
    When parse_multi_panel_input is called
    Then btn_ap() SHALL be true
    And btn_hdg() SHALL be true
    And btn_nav() SHALL be true
    And btn_ias() SHALL be true
    And btn_alt() SHALL be true
    And btn_vs() SHALL be true
    And btn_apr() SHALL be true
    And btn_rev() SHALL be true

  @AC-94.4
  Scenario: Report shorter than MULTI_PANEL_INPUT_MIN_BYTES (3) returns None
    Given a HID input buffer of length 0, 1, or 2
    When parse_multi_panel_input is called
    Then the result SHALL be None for every short buffer
