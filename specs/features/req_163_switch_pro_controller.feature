@REQ-163 @product
Feature: Nintendo Switch Pro Controller flight support

  @AC-163.1
  Scenario: Left stick X/Y axes decoded
    Given a Nintendo Switch Pro Controller is connected and bound
    When the left stick is moved on its X and Y axes
    Then both axes SHALL be decoded and normalized to [-1.0, 1.0]

  @AC-163.2
  Scenario: Right stick X/Y axes decoded
    Given a Nintendo Switch Pro Controller is connected and bound
    When the right stick is moved on its X and Y axes
    Then both axes SHALL be decoded and normalized to [-1.0, 1.0]

  @AC-163.3
  Scenario: Trigger buttons L/R decoded
    Given a Nintendo Switch Pro Controller is connected and bound
    When the L or R trigger button is pressed
    Then the corresponding button event SHALL be emitted

  @AC-163.4
  Scenario: ZL/ZR shoulder buttons decoded
    Given a Nintendo Switch Pro Controller is connected and bound
    When the ZL or ZR shoulder button is pressed
    Then the corresponding button event SHALL be emitted

  @AC-163.5
  Scenario: Gyroscope data available when motion control enabled
    Given a Nintendo Switch Pro Controller is connected and bound
    And motion control is enabled in the profile
    When the controller is physically rotated
    Then gyroscope data SHALL be available as optional axis inputs

  @AC-163.6
  Scenario: Device identified by VID 0x057E PID 0x2009
    Given the HID subsystem is running
    When a device with VID 0x057E and PID 0x2009 is connected
    Then the device SHALL be identified as a Nintendo Switch Pro Controller and offered for flight profile binding
