@REQ-454 @product
Feature: Device Capability Query — Report Connected Device Capabilities via IPC

  @AC-454.1
  Scenario: DeviceInfo IPC response includes axis count, button count, and FFB support
    Given a connected joystick with 6 axes, 32 buttons, and FFB support
    When a DeviceInfo IPC request is made for that device
    Then the response SHALL include axis_count=6, button_count=32, and ffb_supported=true

  @AC-454.2
  Scenario: Capability query returns results within 50ms
    Given a running service with at least one connected device
    When a capability query IPC request is issued
    Then the response SHALL be received within 50 milliseconds

  @AC-454.3
  Scenario: Disconnected devices are reported with last-known capabilities
    Given a device that was previously connected and then disconnected
    When a DeviceInfo IPC request is made for that device ID
    Then the response SHALL include the last-known capabilities and a disconnected status flag

  @AC-454.4
  Scenario: New device connection triggers capability update notification
    Given a client subscribed to device capability update notifications
    When a new HID device is connected
    Then the client SHALL receive a capability update notification within 500ms of connection
