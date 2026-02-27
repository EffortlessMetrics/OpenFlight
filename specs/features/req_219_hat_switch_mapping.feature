@REQ-219 @product
Feature: Hat switches and POV controls map to discrete events or view axis pairs  @AC-219.1
  Scenario: Hat switch 8-directional positions decoded from HID value
    Given a HID device with a hat switch reporting values 0 through 8
    When each value is processed by the hat switch decoder
    Then values 0-7 SHALL map to cardinal and intercardinal directions and value 8 SHALL indicate released  @AC-219.2
  Scenario: Hat switch maps to button events
    Given a hat switch mapped to button events in the active profile
    When the hat switch moves to the North position
    Then a discrete button event for the N direction SHALL be emitted on the bus  @AC-219.3
  Scenario: Hat switch maps to virtual axis pair for view control
    Given a hat switch mapped to a virtual X/Y axis pair for view control in the active profile
    When the hat switch moves to the East position
    Then the virtual X axis SHALL be set to +1.0 and the virtual Y axis SHALL remain at 0.0  @AC-219.4
  Scenario: Multiple hat switches on same device handled independently
    Given a device with two hat switches both active simultaneously
    When hat switch 1 moves North and hat switch 2 moves East at the same time
    Then events for both hat switches SHALL be emitted independently with no cross-contamination  @AC-219.5
  Scenario: Hat switch mapping configurable per axis in profile
    Given a profile with a specific hat switch mapping configuration for an axis
    When the profile is loaded and applied to the device
    Then the hat switch behaviour SHALL match the mapping specified in the profile  @AC-219.6
  Scenario: Hat switch state emitted as raw value and decoded direction
    Given a hat switch is active and connected
    When the hat switch position changes
    Then the bus SHALL carry both the raw HID value and the decoded direction enum for each state change
