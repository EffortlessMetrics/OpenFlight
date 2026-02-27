@REQ-149 @product
Feature: RealSimGear avionics panels  @AC-149.1
  Scenario: G1000 PFD bezel knob CW increments value
    Given a RealSimGear G1000 PFD panel connected via USB HID
    When the bezel knob is rotated clockwise by one detent
    Then the associated value SHALL be incremented by one step  @AC-149.2
  Scenario: G1000 MFD soft keys decoded
    Given a RealSimGear G1000 MFD panel connected via USB HID
    When a HID input report with soft key presses is received
    Then all soft key states SHALL be decoded and reported correctly  @AC-149.3
  Scenario: GNS 530 outer knob rotation
    Given a RealSimGear GNS 530 panel connected via USB HID
    When the outer concentric knob is rotated
    Then the adapter SHALL report the correct direction and step count  @AC-149.4
  Scenario: GNS 430W button states decoded
    Given a RealSimGear GNS 430W panel connected via USB HID
    When a HID input report with button states is received
    Then all button states SHALL be decoded correctly  @AC-149.5
  Scenario: KAP 140 autopilot button press
    Given a RealSimGear KAP 140 autopilot panel connected via USB HID
    When an autopilot button is pressed
    Then the adapter SHALL emit the corresponding button event  @AC-149.6
  Scenario: Panel connects over USB HID
    Given a RealSimGear avionics panel plugged into a USB port
    When HID enumeration runs
    Then the panel SHALL appear in the enumerated HID device list  @AC-149.7
  Scenario: LED and display update written to device
    Given a connected RealSimGear panel that supports LED or display output
    When a display update command is issued
    Then the adapter SHALL write the correct HID output report to the device
