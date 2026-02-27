@REQ-368 @product
Feature: Device Reconnect Policy — Automatic Re-Apply Profile on Reconnect

  @AC-368.1
  Scenario: Profile is re-applied within 500 ms of reconnect
    Given a device that was previously connected and configured
    When the device reconnects after a disconnection event
    Then the profile SHALL be re-applied to the device within 500 ms

  @AC-368.2
  Scenario: Calibration data is restored on reconnect
    Given a device with saved calibration data from a previous session
    When the device reconnects
    Then the calibration data SHALL be restored and applied to the device

  @AC-368.3
  Scenario: Firmware change triggers warning but profile still applies
    Given a device whose firmware version has changed since last connection
    When the device reconnects
    Then a warning SHALL be logged and the profile SHALL still be applied

  @AC-368.4
  Scenario: Device is marked failed after 3 reconnect retries
    Given a device that fails to reconnect successfully
    When 3 reconnect attempts have been made without success
    Then the device SHALL be marked as failed and no further retries attempted

  @AC-368.5
  Scenario: Reconnect attempts are logged with device ID and attempt number
    Given a device that requires multiple reconnect attempts
    When each reconnect attempt is made
    Then each attempt SHALL be logged with the device ID and attempt number

  @AC-368.6
  Scenario: Device disconnects mid-flight and profile is restored on reconnect
    Given a connected device with an active profile during a flight session
    When the device is disconnected and then reconnected
    Then the profile SHALL be automatically restored and axis processing resumed
