@REQ-519 @product
Feature: Device Connection Timeout Config

  @AC-519.1 @AC-519.2
  Scenario: Configurable timeout triggers reconnect retry with backoff
    Given a device profile specifying a connection timeout of 5 seconds
    When the device fails to respond within 5 seconds
    Then the service SHALL retry connection with exponential backoff

  @AC-519.3
  Scenario: Maximum retry count is configurable per device
    Given a device profile with max retry count set to 3
    When the device fails to connect after 3 retry attempts
    Then no further retries SHALL be attempted for that device

  @AC-519.4
  Scenario: Persistent connection failure logs device info and disables polling
    Given a device that has exceeded its maximum retry count
    When all retries are exhausted
    Then the service SHALL log the device VID, PID, and serial number and disable polling
