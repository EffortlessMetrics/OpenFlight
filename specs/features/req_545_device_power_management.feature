Feature: Device Power Management
  As a flight simulation enthusiast
  I want the service to handle device power state changes gracefully
  So that devices resume correctly after sleep or power events

  Background:
    Given the OpenFlight service is running
    And a HID joystick is connected and active

  Scenario: Device sleep triggers safe disconnection handling
    When the operating system signals that the joystick is entering sleep/suspend
    Then the service marks the device as safely disconnected
    And the axis pipeline switches to neutral hold for that device

  Scenario: Device wake triggers re-enumeration and configuration restore
    When the joystick wakes from sleep
    Then the service re-enumerates the device
    And the device's profile configuration is restored automatically

  Scenario: Power events published on flight-bus
    When the joystick enters sleep
    Then a "DevicePowerSleep" event is published on the flight-bus
    When the joystick wakes
    Then a "DevicePowerWake" event is published on the flight-bus
