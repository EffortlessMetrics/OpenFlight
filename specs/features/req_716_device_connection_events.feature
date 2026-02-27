Feature: Device Connection Events
  As a flight simulation enthusiast
  I want the service to emit events on device connect and disconnect
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Event emitted on device connect
    Given the service is monitoring for device changes
    When a new HID device is connected
    Then a device connected event is emitted

  Scenario: Event emitted on device disconnect
    Given a device is currently connected
    When the device is disconnected
    Then a device disconnected event is emitted

  Scenario: Events include device identity
    Given a device connection event occurs
    When the event is received
    Then it includes the device VID, PID, and name

  Scenario: Events delivered to IPC subscribers
    Given an IPC client is subscribed to device events
    When a device connect or disconnect event occurs
    Then the event is delivered to all subscribed IPC clients
