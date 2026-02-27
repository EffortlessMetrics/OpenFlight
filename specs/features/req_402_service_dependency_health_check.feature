@REQ-402 @product
Feature: Service Dependency Health Check — Verify Dependencies Before Starting

  @AC-402.1
  Scenario: Service checks HID subsystem, network interfaces, and config file on startup
    Given a service startup sequence
    When the dependency health check runs
    Then it SHALL check the HID subsystem, network interfaces, and config file

  @AC-402.2
  Scenario: Missing critical dependencies abort startup with a descriptive error
    Given a critical dependency that is unavailable at startup
    When the health check detects the missing dependency
    Then startup SHALL be aborted with a descriptive error message

  @AC-402.3
  Scenario: Optional dependencies log warnings but do not block startup
    Given an optional dependency (such as a sim adapter) that is unavailable
    When the health check detects the missing optional dependency
    Then a warning SHALL be logged and startup SHALL continue

  @AC-402.4
  Scenario: Health check results are logged and available via flightctl health
    Given a completed startup health check
    When the user runs `flightctl health`
    Then the health check results SHALL be returned, matching those written to the log

  @AC-402.5
  Scenario: Health check completes within 2 seconds
    Given a service starting under normal conditions
    When the dependency health check is timed
    Then it SHALL complete within 2 seconds

  @AC-402.6
  Scenario: Each dependency check has an individual timeout of 500 ms
    Given a dependency check that hangs
    When the individual check timeout is reached
    Then that check SHALL be abandoned after 500 ms and marked as timed out
