@REQ-352 @hardware @hotplug @hid
Feature: Device hot-plug detection
  As a user who connects or disconnects devices while the service is running
  I want the service to detect changes without restarting
  So that my setup is always current without manual intervention

  Scenario: USB connect detected within 500 ms on Windows  @AC-352.1
    Given the service is running on Windows
    When a USB HID device is plugged in
    Then a device-connected event SHALL be raised within 500 ms

  Scenario: USB connect detected within 500 ms on Linux  @AC-352.2
    Given the service is running on Linux
    When a USB HID device is plugged in
    Then a device-connected event SHALL be raised within 500 ms via udev monitor

  Scenario: Reconnection restores prior configuration  @AC-352.3
    Given a device with a saved configuration is disconnected
    When the same device is reconnected
    Then its prior axis mappings and settings SHALL be restored automatically

  Scenario: Profile re-applies to reconnected device  @AC-352.4
    Given a profile is active with bindings for device "VID_1234:PID_5678"
    When that device is disconnected and reconnected
    Then the profile SHALL be re-applied to the device

  Scenario: Hot-plug events are logged  @AC-352.5
    Given the service log level is info or higher
    When any device connects or disconnects
    Then a log entry SHALL be written containing the timestamp and device identity

  Scenario: Simultaneous hot-plug events handled safely  @AC-352.6
    Given three devices are connected simultaneously
    When all three connect events arrive at the same instant
    Then all three devices SHALL be registered without data races or panics
