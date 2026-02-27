Feature: Service SIGTERM Handler
  As a system operator
  I want the OpenFlight service to handle SIGTERM gracefully on Unix
  So that the service shuts down cleanly without data loss

  Background:
    Given the OpenFlight service is running on a Unix system

  Scenario: SIGTERM triggers graceful shutdown sequence
    When the service receives a SIGTERM signal
    Then the graceful shutdown sequence is initiated

  Scenario: Shutdown completes within configured timeout
    Given the shutdown timeout is configured
    When the service receives a SIGTERM signal
    Then the shutdown sequence completes within the configured timeout

  Scenario: All adapters are disconnected before process exits
    When the service receives a SIGTERM signal and begins shutdown
    Then all simulator and hardware adapters are disconnected before the process exits

  Scenario: Final state is flushed to disk before exit
    When the service receives a SIGTERM signal and begins shutdown
    Then the final service state is flushed to disk before the process exits
