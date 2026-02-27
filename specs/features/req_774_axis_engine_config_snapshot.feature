Feature: Axis Engine Config Snapshot
  As a flight simulation enthusiast
  I want axis engine config snapshot
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Snapshot at tick boundary
    Given the system is configured for axis engine config snapshot
    When the feature is exercised
    Then axis engine configuration is snapshottable at any tick boundary

  Scenario: Capture all mappings and parameters
    Given the system is configured for axis engine config snapshot
    When the feature is exercised
    Then snapshot captures all axis mappings and processing parameters

  Scenario: Exportable for diagnostics
    Given the system is configured for axis engine config snapshot
    When the feature is exercised
    Then snapshot is exportable for diagnostic purposes

  Scenario: Non-blocking snapshot operation
    Given the system is configured for axis engine config snapshot
    When the feature is exercised
    Then snapshot operation does not block the rt path
