Feature: Service Event Journal
  As a flight simulation enthusiast
  I want service event journal
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: All service events are persisted to a rotated journal file
    Given the system is configured for service event journal
    When the feature is exercised
    Then all service events are persisted to a rotated journal file

  Scenario: Journal entries include timestamp, severity, source, and payload
    Given the system is configured for service event journal
    When the feature is exercised
    Then journal entries include timestamp, severity, source, and payload

  Scenario: Journal rotation triggers based on file size or time interval
    Given the system is configured for service event journal
    When the feature is exercised
    Then journal rotation triggers based on file size or time interval

  Scenario: Journal can be queried by time range, severity, or source filter
    Given the system is configured for service event journal
    When the feature is exercised
    Then journal can be queried by time range, severity, or source filter
