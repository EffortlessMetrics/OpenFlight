Feature: Crash Recovery
  As a flight simulation enthusiast
  I want crash recovery
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Service auto-restarts after unexpected termination with state restoration
    Given the system is configured for crash recovery
    When the feature is exercised
    Then service auto-restarts after unexpected termination with state restoration

  Scenario: Previous session state is recovered from persistent checkpoint data
    Given the system is configured for crash recovery
    When the feature is exercised
    Then previous session state is recovered from persistent checkpoint data

  Scenario: Crash count tracking prevents restart loops with exponential backoff
    Given the system is configured for crash recovery
    When the feature is exercised
    Then crash count tracking prevents restart loops with exponential backoff

  Scenario: Crash dump is captured and stored for diagnostic analysis
    Given the system is configured for crash recovery
    When the feature is exercised
    Then crash dump is captured and stored for diagnostic analysis