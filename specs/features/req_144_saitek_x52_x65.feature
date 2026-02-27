@REQ-144 @product
Feature: Saitek X52 and X65 HOTAS  @AC-144.1
  Scenario: X52 Pro stick axes parsed at 12-bit resolution
    Given a Saitek X52 Pro connected and producing HID reports
    When a stick report with X value 0xABC and Y value 0x123 is received
    Then the adapter SHALL parse X and Y axis values at 12-bit resolution  @AC-144.2
  Scenario: X52 Pro throttle axis parsed
    Given a Saitek X52 Pro connected and producing HID reports
    When a throttle report with value 0x7FF is received
    Then the adapter SHALL parse the throttle axis value at 12-bit resolution  @AC-144.3
  Scenario: X65F stick axes parsed at 14-bit resolution
    Given a Saitek X65F connected and producing HID reports
    When a stick report with X value 0x3ABC and Y value 0x0FFF is received
    Then the adapter SHALL parse X and Y axis values at 14-bit resolution  @AC-144.4
  Scenario: X65F Mode switch decoded
    Given a Saitek X65F connected and producing HID reports
    When the mode switch is set to position 2
    Then the adapter SHALL decode mode switch state as 2  @AC-144.5
  Scenario: LED control sets the requested colour
    Given a Saitek X52 Pro connected with LED control support
    When the profile requests LED colour red for button 1
    Then the device LED SHALL be set to red  @AC-144.6
  Scenario: MFD display writes text correctly
    Given a Saitek X52 Pro with a multi-function display
    When the adapter writes the string "ALT 10000" to line 1
    Then the MFD display SHALL show "ALT 10000" on line 1  @AC-144.7
  Scenario: Clutch button enables pedal mode
    Given a Saitek X52 Pro with clutch button support
    When the clutch button is held
    Then the adapter SHALL activate pedal-mode axis mapping  @AC-144.8
  Scenario: Device identified by VID/PID
    Given a HID device enumeration result
    When a device with the Saitek X52 Pro VID/PID pair is present
    Then the adapter SHALL identify it as a Saitek X52 Pro
