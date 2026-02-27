Feature: Axis History Export
  As a flight simulation enthusiast
  I want axis history export
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Export history buffer
    Given the system is configured for axis history export
    When the feature is exercised
    Then axis history buffer contents are exportable for analysis

  Scenario: Include raw processed timestamps
    Given the system is configured for axis history export
    When the feature is exercised
    Then export includes raw input, processed output, and timestamps

  Scenario: CSV and JSON format support
    Given the system is configured for axis history export
    When the feature is exercised
    Then export format supports csv and json

  Scenario: Non-blocking export
    Given the system is configured for axis history export
    When the feature is exercised
    Then export does not block the rt processing path
