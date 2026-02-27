Feature: Profile Merge Conflict Resolution
  As a flight simulation enthusiast
  I want profile merge conflict resolution
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Most-specific-wins resolution
    Given the system is configured for profile merge conflict resolution
    When the feature is exercised
    Then profile merge resolves conflicts using most-specific-wins rule

  Scenario: Log conflicting keys with sources
    Given the system is configured for profile merge conflict resolution
    When the feature is exercised
    Then conflicting keys are logged with their source profiles

  Scenario: Deterministic merge result
    Given the system is configured for profile merge conflict resolution
    When the feature is exercised
    Then merge result is deterministic for the same input set

  Scenario: Override markers bypass resolution
    Given the system is configured for profile merge conflict resolution
    When the feature is exercised
    Then explicit override markers bypass conflict resolution
