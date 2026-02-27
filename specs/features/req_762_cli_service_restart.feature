Feature: CLI Service Restart Command
  As a flight simulation enthusiast
  I want cli service restart command
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Restart subcommand
    Given the system is configured for cli service restart command
    When the feature is exercised
    Then cli provides a restart subcommand for the service

  Scenario: Graceful shutdown before restart
    Given the system is configured for cli service restart command
    When the feature is exercised
    Then restart waits for graceful shutdown before restarting

  Scenario: Preserve device assignments
    Given the system is configured for cli service restart command
    When the feature is exercised
    Then restart preserves device assignments

  Scenario: Configurable restart timeout
    Given the system is configured for cli service restart command
    When the feature is exercised
    Then restart timeout is configurable
