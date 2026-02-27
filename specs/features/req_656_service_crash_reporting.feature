Feature: Service Crash Reporting
  As a flight simulation enthusiast
  I want the service to generate structured crash reports on panic
  So that I can diagnose and report issues after unexpected failures

  Background:
    Given the OpenFlight service is running

  Scenario: Panic hook captures backtrace and last known state
    When the service encounters a panic
    Then the panic hook captures the full backtrace and last known service state

  Scenario: Crash report is written to configurable crash directory
    Given a crash directory is configured
    When a panic occurs
    Then a crash report file is written to the configured crash directory

  Scenario: Crash report includes service version and config hash
    When a crash report is generated
    Then the report contains the service version and a hash of the active configuration

  Scenario: Next service startup detects prior crash and logs notice
    Given a crash report exists from a previous run
    When the service starts up
    Then it detects the prior crash report and logs a visible notice
