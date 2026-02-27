Feature: Bus Replay from File
  As a flight simulation enthusiast
  I want bus replay from file
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Load and replay messages from file
    Given the system is configured for bus replay from file
    When the feature is exercised
    Then bus supports loading and replaying a recorded message sequence from file

  Scenario: Preserve timing and ordering
    Given the system is configured for bus replay from file
    When the feature is exercised
    Then replay preserves original message timing and ordering

  Scenario: Adjustable replay speed multiplier
    Given the system is configured for bus replay from file
    When the feature is exercised
    Then replay speed is adjustable via a multiplier parameter

  Scenario: Signal completion via bus event
    Given the system is configured for bus replay from file
    When the feature is exercised
    Then replay completion is signaled via a bus event
