@REQ-276 @product
Feature: Plug-and-play detection auto-binds known devices and logs all PnP events with VID/PID  @AC-276.1
  Scenario: New USB device is detected within one second of plug-in
    Given the service is running and monitoring USB events
    When a USB HID device is physically connected to the host
    Then the service SHALL detect the new device within 1 second of plug-in  @AC-276.2
  Scenario: Known device is automatically bound to its last profile
    Given a device with a prior profile binding has been disconnected
    When the same device is reconnected by VID and PID
    Then the service SHALL automatically restore the profile that was last bound to that device  @AC-276.3
  Scenario: Unknown device appears in diagnostics as unrecognized
    Given a USB HID device with no registered driver or profile is connected
    When the diagnostic device list is queried
    Then the device SHALL appear with a status of unrecognized  @AC-276.4
  Scenario: Device removal triggers idle axis state within 100 ms
    Given a device is connected and actively producing axis values
    When the device is physically disconnected
    Then the service SHALL transition all axes from that device to the idle state within 100 ms  @AC-276.5
  Scenario: Rapid plug and unplug cycle does not crash or leak resources
    Given the service is running
    When a USB device is connected and disconnected repeatedly in quick succession
    Then the service SHALL remain stable and not leak file handles or memory  @AC-276.6
  Scenario: PnP events are logged with VID PID and device name
    Given the service has structured logging enabled
    When a USB device is connected or disconnected
    Then a log entry SHALL be written containing the device VID, PID, and display name
