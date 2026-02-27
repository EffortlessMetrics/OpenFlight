@REQ-115 @product
Feature: WinWing UFC Panel device support

  @AC-115.1
  Scenario: Button matrix decodes each button independently
    Given a WinWing UFC Panel HID report with exactly one button bit set
    When the report is parsed
    Then only that button SHALL be reported as pressed
    And all other buttons SHALL be reported as not pressed

  @AC-115.1
  Scenario: All buttons simultaneously pressed are all reported
    Given a WinWing UFC Panel HID report with all button bits set
    When the report is parsed
    Then every button SHALL be reported as pressed

  @AC-115.2
  Scenario: Set LED command encodes the correct LED index and state
    Given a WinWing UFC Panel device driver
    When set_led is called with LED index 3 and state ON
    Then the output HID report SHALL have the LED-3 bit set in the LED bitmask

  @AC-115.2
  Scenario: Clear LED command encodes the correct LED index and state
    Given a WinWing UFC Panel device driver with LED 5 currently on
    When set_led is called with LED index 5 and state OFF
    Then the output HID report SHALL have the LED-5 bit cleared in the LED bitmask

  @AC-115.3
  Scenario: 7-segment display output encoding for digit zero
    Given a WinWing UFC Panel device driver
    When write_display is called with the digit 0 on display position 1
    Then the encoded segment byte SHALL match the standard 7-segment encoding for 0

  @AC-115.3
  Scenario: 7-segment display output encoding for all digits 0-9 is valid
    Given a WinWing UFC Panel device driver
    When write_display is called with each digit 0 through 9
    Then each encoded segment byte SHALL be a valid non-zero 7-segment pattern

  @AC-115.4
  Scenario: LED state read-back matches the last set command
    Given a WinWing UFC Panel device driver with LED 2 set to ON
    When led_state is queried for LED index 2
    Then the returned state SHALL be ON

  @AC-115.5
  Scenario: Panel button events are published on the bus event stream
    Given a WinWing UFC Panel connected to the flight bus
    When a button press is detected in the HID report
    Then a PanelButtonEvent SHALL be published on the bus with the correct button index and state
