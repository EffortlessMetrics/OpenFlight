@REQ-56
Feature: Honeycomb Aeronautical HOTAS input parsing and LED control

  @AC-56.1
  Scenario: Alpha Yoke reports centered position at neutral
    Given an Alpha Yoke HID report with roll=2048 pitch=2048 and no buttons
    When the report is parsed
    Then roll SHALL be approximately 0.0
    And pitch SHALL be approximately 0.0
    And hat SHALL be 0 (centred)

  @AC-56.1
  Scenario: Alpha Yoke reports full right roll
    Given an Alpha Yoke HID report with roll=4095
    When the report is parsed
    Then roll SHALL be approximately +1.0

  @AC-56.1
  Scenario: Alpha Yoke reports full left roll
    Given an Alpha Yoke HID report with roll=0
    When the report is parsed
    Then roll SHALL be approximately -1.0

  @AC-56.1
  Scenario: Alpha Yoke reports full forward pitch
    Given an Alpha Yoke HID report with pitch=0
    When the report is parsed
    Then pitch SHALL be approximately -1.0

  @AC-56.2
  Scenario: Alpha Yoke detects individual button presses
    Given an Alpha Yoke HID report with button 1 and button 36 pressed
    When the report is parsed
    Then is_pressed(1) SHALL return true
    And is_pressed(2) SHALL return false
    And is_pressed(36) SHALL return true

  @AC-56.2
  Scenario: Alpha Yoke decodes hat direction North
    Given an Alpha Yoke HID report with hat raw nibble 0
    When the report is parsed
    Then hat SHALL be 1
    And hat_direction SHALL return "N"

  @AC-56.2
  Scenario: Alpha Yoke decodes hat centred
    Given an Alpha Yoke HID report with hat raw nibble 15
    When the report is parsed
    Then hat SHALL be 0
    And hat_direction SHALL return "center"

  @AC-56.3
  Scenario: Alpha Yoke parse error on short report
    Given a raw HID buffer shorter than 11 bytes
    When parse_alpha_report is called
    Then a TooShort error SHALL be returned

  @AC-56.3
  Scenario: Alpha Yoke parse error on unknown report ID
    Given a raw HID buffer of 11 bytes with first byte 0x02
    When parse_alpha_report is called
    Then an UnknownReportId error SHALL be returned

  @AC-56.3
  Scenario: Bravo Throttle parse error on short report
    Given a raw HID buffer shorter than 23 bytes
    When parse_bravo_report is called
    Then a TooShort error SHALL be returned

  @AC-56.3
  Scenario: Bravo Throttle parse error on unknown report ID
    Given a raw HID buffer of 23 bytes with first byte 0x02
    When parse_bravo_report is called
    Then an UnknownReportId error SHALL be returned

  @AC-56.4
  Scenario: Bravo Throttle all axes at minimum
    Given a Bravo Throttle HID report with all axis raw values 0
    When the report is parsed
    Then throttle1, throttle5, flap_lever, and spoiler SHALL all be approximately 0.0

  @AC-56.4
  Scenario: Bravo Throttle all axes at maximum
    Given a Bravo Throttle HID report with all axis raw values 4095
    When the report is parsed
    Then throttle1, flap_lever, and spoiler SHALL all be approximately 1.0

  @AC-56.4
  Scenario: Bravo Throttle gear-up button detected
    Given a Bravo Throttle HID report with bit 30 set in the button mask
    When the report is parsed
    Then gear_up SHALL return true
    And gear_down SHALL return false

  @AC-56.4
  Scenario: Bravo Throttle AP master button detected
    Given a Bravo Throttle HID report with bit 7 set in the button mask
    When the report is parsed
    Then ap_master SHALL return true

  @AC-56.5
  Scenario: All-off LED state produces zero data bytes
    Given a BravoLedState with all LEDs off
    When serialize_led_report is called
    Then report byte 0 SHALL be 0x00
    And bytes 1 through 4 SHALL all be 0

  @AC-56.5
  Scenario: HDG LED encodes to bit 0 of byte 1
    Given a BravoLedState with only the HDG LED enabled
    When serialize_led_report is called
    Then byte 1 SHALL equal 0b00000001

  @AC-56.5
  Scenario: set_all_gear green sets green bits and clears red bits
    Given a BravoLedState after set_all_gear(true)
    When serialize_led_report is called
    Then bits 0, 2, 4 of byte 2 SHALL be set (L/C/R green)
    And bits 1, 3, 5 of byte 2 SHALL be clear (L/C/R red)

  @AC-56.5
  Scenario: All-on LED state round-trips correctly
    Given a BravoLedState with all LEDs on
    When serialize_led_report is called
    Then byte 0 SHALL be 0x00
    And byte 1 SHALL be 0xFF
    And byte 2 SHALL be 0xFF
    And byte 3 SHALL be 0xFF
    And the high nibble of byte 4 SHALL be 0 (unused bits)
