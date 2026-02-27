Feature: Diagnostic Commands
  As a flight simulation enthusiast
  I want diagnostic commands
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: CLI provides diagnostic commands for system health inspection
    Given the system is configured for diagnostic commands
    When the feature is exercised
    Then cLI provides diagnostic commands for system health inspection

  Scenario: Diagnostics include device enumeration, connection status, and axis state
    Given the system is configured for diagnostic commands
    When the feature is exercised
    Then diagnostics include device enumeration, connection status, and axis state

  Scenario: Diagnostic output is structured for both human and machine consumption
    Given the system is configured for diagnostic commands
    When the feature is exercised
    Then diagnostic output is structured for both human and machine consumption

  Scenario: Diagnostic bundle export collects logs, config, and state into single archive
    Given the system is configured for diagnostic commands
    When the feature is exercised
    Then diagnostic bundle export collects logs, config, and state into single archive
