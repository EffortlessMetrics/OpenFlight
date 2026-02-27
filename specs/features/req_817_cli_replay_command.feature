Feature: CLI Replay Command
  As a flight simulation enthusiast
  I want cli replay command
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Load and replay recorded axis trace
    Given the system is configured for cli replay command
    When the feature is exercised
    Then cLI loads a recorded axis trace file and replays it through the pipeline

  Scenario: Adjustable replay speed multiplier
    Given the system is configured for cli replay command
    When the feature is exercised
    Then replay speed is adjustable with a multiplier parameter

  Scenario: Output to file or stdout
    Given the system is configured for cli replay command
    When the feature is exercised
    Then replay output is written to a file or streamed to stdout

  Scenario: Preserve original timing relationships
    Given the system is configured for cli replay command
    When the feature is exercised
    Then replay preserves original timing relationships between samples
