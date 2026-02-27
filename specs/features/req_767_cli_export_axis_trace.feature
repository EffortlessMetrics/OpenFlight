Feature: CLI Export Axis Trace
  As a flight simulation enthusiast
  I want cli export axis trace
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Export to CSV format
    Given the system is configured for cli export axis trace
    When the feature is exercised
    Then cli exports axis trace data from blackbox to csv format

  Scenario: Include timestamp raw and processed columns
    Given the system is configured for cli export axis trace
    When the feature is exercised
    Then export includes timestamp, raw input, and processed output columns

  Scenario: Filterable time range
    Given the system is configured for cli export axis trace
    When the feature is exercised
    Then time range is filterable with start and end parameters

  Scenario: Select specific axes
    Given the system is configured for cli export axis trace
    When the feature is exercised
    Then export supports selecting specific axes by name
