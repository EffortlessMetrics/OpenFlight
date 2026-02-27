@REQ-406 @product
Feature: Service Start/Stop Command — Controlled Lifecycle via CLI

  @AC-406.1
  Scenario: flightctl service start starts the service daemon
    Given the service is not running
    When the user runs `flightctl service start`
    Then the service daemon SHALL start and be in the running state

  @AC-406.2
  Scenario: flightctl service stop gracefully shuts down the service
    Given the service is running
    When the user runs `flightctl service stop`
    Then the service SHALL perform a graceful shutdown

  @AC-406.3
  Scenario: flightctl service restart stops and then starts
    Given the service is running
    When the user runs `flightctl service restart`
    Then the service SHALL stop and then start again

  @AC-406.4
  Scenario: flightctl service status shows running/stopped and PID
    Given the service is in any lifecycle state
    When the user runs `flightctl service status`
    Then the output SHALL show the current state (running/stopped) and the PID if running

  @AC-406.5
  Scenario: Stop command waits up to 5 seconds for graceful shutdown then kills
    Given the service is running but not responding to graceful shutdown
    When `flightctl service stop` is issued
    Then the command SHALL wait up to 5 seconds before forcibly terminating the process

  @AC-406.6
  Scenario: All lifecycle commands return exit code 0 on success and non-zero on failure
    Given any service lifecycle command
    When the command completes successfully
    Then the exit code SHALL be 0
    And when the command fails the exit code SHALL be non-zero
