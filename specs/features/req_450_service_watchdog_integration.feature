@REQ-450 @product
Feature: Service Watchdog Integration — Integrate with System Watchdog for Crash Recovery

  @AC-450.1
  Scenario: Service sends watchdog keep-alive pings at configured interval
    Given the watchdog ping interval is set to 5 seconds
    When the service is running normally
    Then a keep-alive ping SHALL be sent to the watchdog every 5 seconds

  @AC-450.2
  Scenario: Watchdog timeout triggers automatic service restart
    Given the service has stopped sending keep-alive pings
    When the watchdog timeout elapses
    Then the watchdog mechanism SHALL restart the service automatically

  @AC-450.3
  Scenario: Repeated crashes within window escalate to error state
    Given the service has crashed and restarted three times within the backoff window
    When the next restart attempt is evaluated
    Then the service SHALL enter an error state and cease automatic restart attempts

  @AC-450.4
  Scenario: Watchdog health is reported in service status output
    Given the service is running
    When flightctl status is invoked
    Then the output SHALL include watchdog health information including last ping time and ping interval
