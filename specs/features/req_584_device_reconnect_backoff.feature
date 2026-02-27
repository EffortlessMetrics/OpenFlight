Feature: Device Reconnect Backoff
  As a flight simulation enthusiast
  I want the service to use exponential backoff for device reconnection
  So that transient disconnects do not overwhelm the system with reconnect attempts

  Background:
    Given the OpenFlight service is running
    And a HID device has disconnected

  Scenario: Reconnect uses exponential backoff with configurable base delay
    Given the base reconnect delay is configured to 500 milliseconds
    When the first reconnect attempt fails
    Then the next attempt is scheduled after approximately 500 milliseconds
    And each subsequent failure doubles the delay

  Scenario: Maximum backoff delay is configurable
    Given the maximum backoff delay is configured to 30 seconds
    When the backoff delay would exceed 30 seconds
    Then the delay is capped at 30 seconds

  Scenario: Successful reconnect resets backoff to base delay
    Given the current backoff delay is 8 seconds
    When the device reconnects successfully
    Then the backoff delay resets to the configured base delay

  Scenario: Backoff state is visible in device diagnostics
    Given the device is in a reconnect backoff state
    When device diagnostics are queried
    Then the diagnostics include the current backoff delay and attempt count
