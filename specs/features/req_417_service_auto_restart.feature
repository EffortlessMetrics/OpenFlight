@REQ-417 @infra
Feature: Service Auto-Restart on Crash — Watchdog Restarts Service on Unexpected Exit

  @AC-417.1
  Scenario: Watchdog detects service process exit within 2 seconds
    Given the watchdog is monitoring the service
    When the service process exits unexpectedly
    Then the watchdog SHALL detect the exit within 2 seconds

  @AC-417.2
  Scenario: Watchdog restarts service up to 5 times before giving up
    Given the service is crashing repeatedly
    When the watchdog attempts restarts
    Then it SHALL attempt up to 5 restarts before entering failed state

  @AC-417.3
  Scenario: Restart attempts use exponential backoff (1s, 2s, 4s, 8s, 16s)
    Given the watchdog performing consecutive restart attempts
    When the delays between restarts are measured
    Then they SHALL follow the exponential sequence: 1 s, 2 s, 4 s, 8 s, 16 s

  @AC-417.4
  Scenario: After 5 failures watchdog enters failed state and logs a critical alert
    Given the watchdog has exhausted all 5 restart attempts
    When the final restart also fails
    Then the watchdog SHALL enter the failed state and emit a CRITICAL-level log alert

  @AC-417.5
  Scenario: Watchdog state is visible via flightctl watchdog status
    Given a watchdog in any state (running, restarting, failed)
    When `flightctl watchdog status` is executed
    Then the current watchdog state SHALL be displayed

  @AC-417.6
  Scenario: Successful restart increments a restart_count metric
    Given a service that has been restarted successfully by the watchdog
    When the metrics are queried
    Then the restart_count metric SHALL reflect the number of successful restarts
