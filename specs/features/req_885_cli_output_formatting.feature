Feature: CLI Output Formatting
  As a flight simulation enthusiast
  I want cli output formatting
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Output format is selectable as table, JSON, or CSV via a flag
    Given the system is configured for cli output formatting
    When the feature is exercised
    Then output format is selectable as table, JSON, or CSV via a flag

  Scenario: Default format is human-readable table for interactive terminals
    Given the system is configured for cli output formatting
    When the feature is exercised
    Then default format is human-readable table for interactive terminals

  Scenario: JSON output conforms to a documented schema for each command
    Given the system is configured for cli output formatting
    When the feature is exercised
    Then jSON output conforms to a documented schema for each command

  Scenario: CSV output includes a header row with column names
    Given the system is configured for cli output formatting
    When the feature is exercised
    Then cSV output includes a header row with column names
