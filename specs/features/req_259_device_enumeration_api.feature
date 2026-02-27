@REQ-259 @product
Feature: gRPC API enumerates all connected HID devices with capabilities  @AC-259.1
  Scenario: ListDevices gRPC call returns all connected HID devices
    Given one or more HID devices are connected and the flightd service is running
    When a client issues a ListDevices gRPC request
    Then the response SHALL contain one entry for every connected HID device  @AC-259.2
  Scenario: Each device entry includes VID PID product name and manufacturer
    Given a connected HID device with known USB descriptor fields
    When ListDevices is called
    Then each device entry SHALL include the vendor ID, product ID, product name, and manufacturer string  @AC-259.3
  Scenario: Axis and button count reported per device
    Given a HID device with a known number of axes and buttons
    When ListDevices is called
    Then the response entry for that device SHALL include the correct axis count and button count  @AC-259.4
  Scenario: Compatibility tier from manifest included in response
    Given a device that has an entry in the device compatibility manifest
    When ListDevices is called
    Then the response SHALL include the compatibility tier for that device as defined in the manifest  @AC-259.5
  Scenario: Device reconnect triggers DeviceConnected server-side event stream
    Given a client subscribed to the device event stream
    When a HID device is disconnected and then reconnected
    Then the server SHALL emit a DeviceConnected event on the stream for the reconnected device  @AC-259.6
  Scenario: flightctl devices CLI uses ListDevices API
    Given the flightd service is running with at least one connected device
    When the user runs flightctl devices
    Then the CLI SHALL call the ListDevices gRPC endpoint and display the returned device list
