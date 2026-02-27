Feature: Profile Version Control
  As a flight simulation enthusiast
  I want profiles to track version history
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Version history tracked
    Given a profile is modified
    When the profile is saved
    Then a new version entry with timestamp is recorded

  Scenario: Previous versions restorable
    Given a profile has multiple versions
    When I run the profile restore command
    Then the selected previous version is restored

  Scenario: Diffs viewable in CLI
    Given a profile has version history
    When I run the profile diff command
    Then the differences between versions are displayed

  Scenario: History bounded to maximum
    Given the version history exceeds the configured maximum
    When a new version is saved
    Then the oldest version is pruned
