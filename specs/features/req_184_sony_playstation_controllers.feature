@REQ-184 @product
Feature: Sony DualShock/DualSense controllers map correctly to flight inputs  @AC-184.1
  Scenario: DualShock 4 detected by USB VID/PID
    Given a Sony DualShock 4 controller is connected via USB
    When the HID subsystem enumerates devices
    Then the controller SHALL be detected by VID 0x054C with PID 0x05C4 or 0x09CC  @AC-184.2
  Scenario: DualSense PS5 controller detected by USB VID/PID
    Given a Sony DualSense PS5 controller is connected via USB
    When the HID subsystem enumerates devices
    Then the controller SHALL be detected by VID 0x054C with PID 0x0CE6  @AC-184.3
  Scenario: Analog sticks normalized to [-1.0, 1.0]
    Given a Sony PlayStation controller is connected and active
    When the analog sticks are moved across their full range of motion
    Then the stick axes SHALL be normalized to the range [-1.0, 1.0]  @AC-184.4
  Scenario: Triggers L2 and R2 normalized to [0.0, 1.0]
    Given a Sony PlayStation controller is connected and active
    When the L2 and R2 triggers are depressed across their full travel
    Then the trigger axes SHALL be normalized to the range [0.0, 1.0]  @AC-184.5
  Scenario: Touchpad click mapped as button event
    Given a Sony PlayStation controller is connected and active
    When the touchpad surface is clicked
    Then a button event SHALL be emitted for the touchpad click  @AC-184.6
  Scenario: Rumble actuators accept FFB commands
    Given a Sony PlayStation controller is connected and active
    When an FFB command is issued from an OpenFlight force feedback profile
    Then the controller's rumble actuators SHALL execute the command as received
