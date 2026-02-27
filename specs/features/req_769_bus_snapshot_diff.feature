Feature: Bus Snapshot Diff
  As a flight simulation enthusiast
  I want bus snapshot diff
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Compute diff between snapshots
    Given the system is configured for bus snapshot diff
    When the feature is exercised
    Then bus supports computing a diff between two state snapshots

  Scenario: Identify added removed changed entries
    Given the system is configured for bus snapshot diff
    When the feature is exercised
    Then diff identifies added, removed, and changed entries

  Scenario: No allocation on RT path
    Given the system is configured for bus snapshot diff
    When the feature is exercised
    Then diff computation does not allocate on the rt path

  Scenario: Serializable diff result
    Given the system is configured for bus snapshot diff
    When the feature is exercised
    Then diff result is serializable for diagnostic export
