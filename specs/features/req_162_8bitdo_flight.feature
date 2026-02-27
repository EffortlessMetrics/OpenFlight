@REQ-162 @product
Feature: 8BitDo controller flight support

  @AC-162.1
  Scenario: Pro 2 left stick mapped to pitch/roll
    Given an 8BitDo Pro 2 controller is connected and a flight profile is loaded
    When the left stick is moved on its X and Y axes
    Then the pitch and roll sim variables SHALL be updated proportionally

  @AC-162.2
  Scenario: Pro 2 right trigger mapped to throttle
    Given an 8BitDo Pro 2 controller is connected and a flight profile is loaded
    When the right trigger is pressed across its full range
    Then the throttle sim variable SHALL be updated proportionally

  @AC-162.3
  Scenario: Button customization via profile
    Given an 8BitDo Pro 2 controller is connected
    And a profile with custom button bindings is loaded
    When a customized button is pressed
    Then the action defined in the profile SHALL be executed

  @AC-162.4
  Scenario: D-pad decoded as hat switch or buttons
    Given an 8BitDo Pro 2 controller is connected and bound
    When the D-pad is actuated in any direction
    Then the input SHALL be decoded as either a hat switch value or discrete button events according to the profile configuration

  @AC-162.5
  Scenario: Wireless reconnect handled
    Given an 8BitDo Pro 2 controller was previously connected and bound
    When the controller reconnects after a wireless dropout
    Then the device SHALL be rebound automatically and input SHALL resume without user intervention

  @AC-162.6
  Scenario: Device identified by VID 0x2DC8
    Given the HID subsystem is running
    When an 8BitDo controller with VID 0x2DC8 is connected
    Then the device SHALL be identified as an 8BitDo controller and offered for flight profile binding
