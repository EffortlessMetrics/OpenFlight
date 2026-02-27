Feature: Device LED Control
  As a flight simulation enthusiast
  I want the service to control device LEDs based on sim state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: LEDs controlled by sim state
    Given a device with LEDs is connected
    When the sim state changes
    Then device LEDs are updated based on the state

  Scenario: LED mappings configurable
    Given LED mappings are defined in the profile
    When the profile is active
    Then LEDs follow the configured mappings

  Scenario: Rate-limited to prevent flicker
    Given LED state changes occur rapidly
    When changes exceed the rate limit
    Then updates are throttled to prevent flicker

  Scenario: Unsupported commands ignored
    Given an LED command targets an unsupported LED
    When the command is processed
    Then it is silently ignored
