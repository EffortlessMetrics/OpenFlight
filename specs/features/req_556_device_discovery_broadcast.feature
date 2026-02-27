@REQ-556 @product
Feature: Device Discovery Broadcast — Service should broadcast device discovery events

  @AC-556.1
  Scenario: Device connect/disconnect is broadcast on flight-bus
    Given the service is running and monitoring HID devices
    When a new HID device is connected
    Then the service SHALL publish a device-connected event on the flight-bus

  @AC-556.2
  Scenario: Bus event includes VID, PID, and device type
    Given a device-connected event on the flight-bus
    When the event payload is inspected
    Then it SHALL contain the device VID, PID, and classified device type

  @AC-556.3
  Scenario: Subscribers can react to device changes in real time
    Given a flight-bus subscriber registered for device events
    When a device is connected or disconnected
    Then the subscriber SHALL receive the event within one bus dispatch cycle

  @AC-556.4
  Scenario: Discovery events have configurable debounce delay
    Given a debounce delay of 200ms is configured for device discovery events
    When a device is rapidly connected and disconnected within 200ms
    Then only one consolidated event SHALL be published on the flight-bus
