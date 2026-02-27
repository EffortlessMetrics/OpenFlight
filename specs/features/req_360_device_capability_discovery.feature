@REQ-360 @product
Feature: Device Capability Discovery  @AC-360.1
  Scenario: Capabilities are enumerated on device connect
    Given a USB HID joystick with 3 axes, 12 buttons, 1 hat, and FFB support is connected
    When the device connect event is processed
    Then the device registry SHALL record 3 axes, 12 buttons, 1 hat, and FFB supported  @AC-360.2
  Scenario: Capabilities are cached in the device registry
    Given a device has been connected and its capabilities enumerated
    When capabilities are queried a second time without reconnection
    Then the result SHALL be served from the registry cache without re-enumerating the device  @AC-360.3
  Scenario: Capability data is available via flightctl devices list verbose
    Given a joystick with known capabilities is connected
    When the user runs "flightctl devices list --verbose"
    Then the output SHALL include the axis count, button count, hat count, and FFB flag  @AC-360.4
  Scenario: Axis and button count match the USB HID descriptor
    Given a device whose HID descriptor declares 6 axes and 8 buttons
    When capabilities are enumerated
    Then the registry SHALL record exactly 6 axes and 8 buttons  @AC-360.5
  Scenario: FFB capability flag is set only for FFB-capable devices
    Given a device that does not include FFB usage pages in its HID descriptor
    When capabilities are enumerated
    Then the FFB capability flag SHALL be false for that device  @AC-360.6
  Scenario: Capability cache is invalidated on firmware version change
    Given a device is connected with firmware version "1.0.0" and its capabilities are cached
    When the device reconnects reporting firmware version "1.1.0"
    Then the capability cache entry SHALL be invalidated and re-enumerated
