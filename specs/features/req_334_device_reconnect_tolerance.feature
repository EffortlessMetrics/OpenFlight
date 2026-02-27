@REQ-334 @product
Feature: Device Reconnect Tolerance  @AC-334.1
  Scenario: Service attempts reconnection 5 times before giving up
    Given a device that fails to reconnect
    When the device disconnects unexpectedly
    Then the service SHALL make exactly 5 reconnect attempts before entering the failed state  @AC-334.2
  Scenario: Each reconnect attempt uses exponential backoff
    Given a device undergoing reconnect attempts
    When successive attempts fail
    Then each waiting interval SHALL be 2× the previous interval (e.g., 1s, 2s, 4s, 8s, 16s)  @AC-334.3
  Scenario: Maximum reconnect interval is capped at 30 seconds
    Given a device that has failed many reconnect attempts
    When the computed backoff interval exceeds 30 seconds
    Then the service SHALL cap the interval at 30 seconds  @AC-334.4
  Scenario: Successful reconnect resets the attempt counter
    Given a device that failed 3 reconnect attempts but then succeeds
    When the device reconnects successfully
    Then the reconnect attempt counter SHALL be reset to 0  @AC-334.5
  Scenario: Reconnect attempts are logged with device details
    Given a device undergoing reconnect
    When a reconnect attempt is made
    Then a log entry SHALL include the device VID/PID, attempt number, and next retry interval  @AC-334.6
  Scenario: Final failure triggers graceful degradation
    Given a device that has exhausted all 5 reconnect attempts
    When the last attempt fails
    Then the service SHALL mark the device unavailable and continue operating other devices without stopping
