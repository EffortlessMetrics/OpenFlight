@REQ-249 @infra
Feature: Service sessions tracked and cleanly terminated on shutdown  @AC-249.1
  Scenario: Service startup generates session ID logged to audit log
    Given the flightd service is starting up
    When the service initialisation completes
    Then a UUID session ID SHALL be generated and written to the audit log  @AC-249.2
  Scenario: Active session duration tracked and available in health response
    Given a service session that has been running for a known duration
    When a health check gRPC call is made
    Then the response SHALL include the active session duration in seconds  @AC-249.3
  Scenario: SIGTERM causes graceful session shutdown within 5 seconds
    Given the flightd service is running an active session
    When a SIGTERM signal is delivered to the process
    Then the service SHALL complete shutdown within 5 seconds  @AC-249.4
  Scenario: Graceful shutdown flushes black box ring buffer to disk
    Given the black box ring buffer contains unwritten telemetry data
    When a graceful shutdown is triggered
    Then the ring buffer contents SHALL be flushed to disk before the process exits  @AC-249.5
  Scenario: Session end event emitted to bus before shutdown completes
    Given the service is processing a shutdown request
    When the session teardown sequence executes
    Then a session-end event SHALL be emitted to the bus before the process terminates  @AC-249.6
  Scenario: Multiple overlapping sessions handled without conflict
    Given a primary session is active and an update process starts a secondary session
    When both sessions attempt to register their session IDs
    Then each session SHALL receive a distinct UUID and neither SHALL overwrite the other's audit entries
