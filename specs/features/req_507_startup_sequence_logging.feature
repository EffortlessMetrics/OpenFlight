@REQ-507 @product
Feature: Service Startup Sequence Logging — Detailed Startup Phase Timings  @AC-507.1
  Scenario: Each startup phase is logged with timestamp and duration
    Given the service is starting
    When startup completes
    Then the log SHALL contain an entry for each startup phase with its start timestamp and elapsed duration  @AC-507.2
  Scenario: Startup log includes device enumeration results
    Given the service starts and enumerates HID devices
    When the startup log is inspected
    Then it SHALL list the name and status of each device found during enumeration  @AC-507.3
  Scenario: Startup failures identify the failing component and reason
    Given a startup phase fails due to a missing configuration file
    When the failure is logged
    Then the log entry SHALL identify the component name and a human-readable failure reason  @AC-507.4
  Scenario: Total startup time is logged and compared to previous run
    Given the service has been started at least once before
    When the service starts again successfully
    Then the log SHALL include the total startup duration and a comparison to the previous run
