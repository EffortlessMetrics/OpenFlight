@REQ-493 @product
Feature: Profile Diff View — Axis-Level Profile Comparison  @AC-493.1
  Scenario: flightctl profile diff shows axis-level changes between profiles
    Given two profiles with differing axis configurations
    When `flightctl profile diff <profile-a> <profile-b>` is executed
    Then the output SHALL list every axis-level difference between the two profiles  @AC-493.2
  Scenario: Added, removed, and modified axes are clearly indicated
    Given a diff between two profiles
    When the diff output is rendered
    Then added axes SHALL be marked with +, removed axes with -, and modified axes with ~  @AC-493.3
  Scenario: Diff can compare active profile against a file
    Given the service is running with an active profile
    When `flightctl profile diff --active <file>` is executed
    Then the diff SHALL compare the active profile to the specified file  @AC-493.4
  Scenario: Diff output supports --json flag
    Given two profiles with differences
    When `flightctl profile diff --json` is executed
    Then the output SHALL be valid JSON describing the axis-level differences
