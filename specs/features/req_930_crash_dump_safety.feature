Feature: Crash Dump Safety
  As a flight simulation enthusiast
  I want crash dump safety
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Crash reports exclude authentication tokens and credentials
    Given the system is configured for crash dump safety
    When the feature is exercised
    Then crash reports exclude authentication tokens and credentials

  Scenario: Memory dumps are scrubbed of user-identifiable profile data
    Given the system is configured for crash dump safety
    When the feature is exercised
    Then memory dumps are scrubbed of user-identifiable profile data

  Scenario: Crash dump upload requires explicit user consent per incident
    Given the system is configured for crash dump safety
    When the feature is exercised
    Then crash dump upload requires explicit user consent per incident

  Scenario: Crash report format is documented for user inspection before submission
    Given the system is configured for crash dump safety
    When the feature is exercised
    Then crash report format is documented for user inspection before submission
