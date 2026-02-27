@REQ-210 @product
Feature: VIRPIL VPC devices fully integrated with 14-bit precision and all features  @AC-210.1
  Scenario: All VIRPIL VPC axes decoded at 14-bit resolution
    Given a VIRPIL VPC device connected via USB
    When axis data is read from the HID report
    Then all axes SHALL be decoded at 14-bit resolution yielding values from 0 to 16383  @AC-210.2
  Scenario: VIRPIL Center of Gravity stick mode handled with different HID layout
    Given a VIRPIL VPC stick configured in Center of Gravity mode
    When HID reports are received
    Then the alternate HID layout SHALL be parsed and axes reported correctly  @AC-210.3
  Scenario: Stick mode configuration bit read from device descriptor at connect
    Given a VIRPIL VPC stick connecting to the service
    When the device descriptor is read at connection time
    Then the stick mode configuration bit SHALL be read and the appropriate parser selected  @AC-210.4
  Scenario: VIRPIL button matrix including encoder modes fully decoded
    Given a VIRPIL VPC device with a button matrix and rotary encoders
    When buttons and encoders are operated
    Then all button matrix states and encoder mode events SHALL be fully decoded  @AC-210.5
  Scenario: Multiple VIRPIL devices assigned to correct roles
    Given a VIRPIL stick, throttle, and pedals all connected simultaneously
    When the service enumerates the devices
    Then each device SHALL be assigned its correct role without role confusion  @AC-210.6
  Scenario: VIRPIL firmware upgrade event handled gracefully
    Given a VIRPIL device undergoing a firmware upgrade
    When the device disconnects and reconnects during the upgrade process
    Then the service SHALL handle the disconnect and reconnect gracefully without crashing
