Feature: Service Log Level Control
  As a flight simulation enthusiast
  I want service log level control
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Runtime log level change via CLI
    Given the system is configured for service log level control
    When the feature is exercised
    Then service log level is changeable at runtime via cli command

  Scenario: Immediate effect without restart
    Given the system is configured for service log level control
    When the feature is exercised
    Then log level changes take effect immediately without restart

  Scenario: Per-module log levels
    Given the system is configured for service log level control
    When the feature is exercised
    Then per-module log levels are supported

  Scenario: Query current log level
    Given the system is configured for service log level control
    When the feature is exercised
    Then current log level is queryable via cli
