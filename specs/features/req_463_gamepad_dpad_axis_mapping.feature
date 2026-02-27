@REQ-463 @product
Feature: Gamepad D-Pad Axis Mapping — Map D-pad to Virtual Axes  @AC-463.1
  Scenario: D-pad buttons are enumerated as hat switch inputs
    Given a gamepad device is connected with a D-pad
    When the HID descriptor is parsed
    Then the D-pad SHALL be enumerated as a hat switch or four directional buttons  @AC-463.2
  Scenario: D-pad is mapped to virtual pitch and roll axes
    Given a profile with D-pad mapped to virtual pitch and roll axes
    When the user presses D-pad up
    Then the virtual pitch axis SHALL receive a positive deflection
    And when the user presses D-pad right the virtual roll axis SHALL receive a positive deflection  @AC-463.3
  Scenario: D-pad axis sensitivity is configurable
    Given a profile with D-pad sensitivity set to 0.5
    When the D-pad is pressed in any direction
    Then the resulting virtual axis value SHALL be scaled by the configured sensitivity factor  @AC-463.4
  Scenario: D-pad supports 8-way diagonal input
    Given a gamepad with D-pad mapped to pitch and roll axes
    When the user presses D-pad up-right simultaneously
    Then both the virtual pitch and roll axes SHALL receive deflections at the same time
