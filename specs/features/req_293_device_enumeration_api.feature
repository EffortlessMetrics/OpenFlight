@REQ-293 @product
Feature: Device Enumeration API  @AC-293.1
  Scenario: gRPC endpoint returns list of all connected HID devices
    Given one or more HID flight control devices are connected to the host
    When the ListDevices gRPC RPC is called
    Then the response SHALL contain an entry for each connected device  @AC-293.2
  Scenario: Device list includes VID/PID, product name, and serial number
    Given a known HID device with vendor ID 0x044F, product ID 0x0404, product name "T.16000M", and serial "SN-001"
    When the ListDevices gRPC RPC is called
    Then the device entry SHALL include the correct vendor ID, product ID, product name, and serial number  @AC-293.3
  Scenario: Device list includes current connection status
    Given a device that was previously connected is now disconnected
    When the ListDevices gRPC RPC is called
    Then the disconnected device's entry SHALL show status "error" or be absent, and active devices SHALL show status "active"  @AC-293.4
  Scenario: API supports filtering by device type
    Given a mix of joystick and throttle devices connected
    When the ListDevices gRPC RPC is called with filter type "joystick"
    Then the response SHALL contain only joystick devices  @AC-293.5
  Scenario: Enumeration available without any profile loaded
    Given the service is running and no profile is loaded
    When the ListDevices gRPC RPC is called
    Then the response SHALL return the device list without error  @AC-293.6
  Scenario: CLI displays device list in table format
    Given at least one HID device is connected
    When the command "flightctl devices list" is run
    Then the output SHALL be formatted as a table with columns for VID, PID, name, serial, and status
