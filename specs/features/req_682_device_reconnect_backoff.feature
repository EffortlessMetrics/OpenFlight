Feature: Device Reconnect Backoff
  As a flight simulation enthusiast
  I want the service to use exponential backoff when reconnecting devices
  So that transient disconnections are handled gracefully without overwhelming the system

  Background:
    Given the OpenFlight service is running

  Scenario: Reconnect attempts use exponential backoff starting at 100ms
    Given a HID device is disconnected
    When the service begins reconnect attempts
    Then the first attempt is after 100ms and each subsequent delay doubles up to a maximum of 30s

  Scenario: Backoff state is logged at each reconnect attempt
    Given a HID device is disconnected and reconnect attempts are ongoing
    When each reconnect attempt occurs
    Then the current backoff delay and attempt number are logged

  Scenario: Reconnect succeeds within two attempts on device restore
    Given a HID device was disconnected and then physically reconnected
    When the service detects the device is available again
    Then reconnection succeeds within two reconnect attempts

  Scenario: Backoff is reset after successful reconnection
    Given a HID device reconnected after a series of failed attempts
    When the reconnection is confirmed successful
    Then the backoff delay is reset to its initial value of 100ms
