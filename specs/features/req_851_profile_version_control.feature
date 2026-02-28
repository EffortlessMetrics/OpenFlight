Feature: Profile Version Control
  As a flight simulation enthusiast
  I want profile version control
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Profile change history is tracked with timestamps and author metadata
    Given the system is configured for profile version control
    When the feature is exercised
    Then profile change history is tracked with timestamps and author metadata

  Scenario: Previous profile versions can be restored from the version log
    Given the system is configured for profile version control
    When the feature is exercised
    Then previous profile versions can be restored from the version log

  Scenario: Version diffs show which axes and bindings changed between revisions
    Given the system is configured for profile version control
    When the feature is exercised
    Then version diffs show which axes and bindings changed between revisions

  Scenario: Version history is persisted across service restarts
    Given the system is configured for profile version control
    When the feature is exercised
    Then version history is persisted across service restarts
