Feature: Service Graceful Shutdown Timeout
  As a flight simulation enthusiast
  I want the service to enforce a graceful shutdown timeout
  So that the service always exits cleanly within a predictable time

  Background:
    Given the OpenFlight service is running with active tasks

  Scenario: Graceful shutdown waits for active tasks to finish
    When the service receives a shutdown signal
    Then it waits for all active tasks to complete before exiting

  Scenario: Tasks that do not finish within timeout are forcefully stopped
    Given the shutdown timeout is 5 seconds
    And a task does not finish within 5 seconds after the shutdown signal
    When the timeout expires
    Then the service forcefully terminates the remaining task and exits

  Scenario: Shutdown timeout is configurable with a default of 5 seconds
    Given no shutdown timeout is set in the service config
    When the service starts
    Then the effective shutdown timeout is 5 seconds
    Given the service config sets shutdown_timeout_secs to 10
    When the service starts
    Then the effective shutdown timeout is 10 seconds

  Scenario: Shutdown sequence is logged with per-task timing
    When the service shuts down
    Then the log contains a shutdown summary entry listing each task and its shutdown duration
