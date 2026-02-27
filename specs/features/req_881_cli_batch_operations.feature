Feature: CLI Batch Operations
  As a flight simulation enthusiast
  I want cli batch operations
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Settings can be applied to multiple devices in a single command
    Given the system is configured for cli batch operations
    When the feature is exercised
    Then settings can be applied to multiple devices in a single command

  Scenario: Batch operations report per-device success or failure status
    Given the system is configured for cli batch operations
    When the feature is exercised
    Then batch operations report per-device success or failure status

  Scenario: A dry-run flag previews changes without applying them
    Given the system is configured for cli batch operations
    When the feature is exercised
    Then a dry-run flag previews changes without applying them

  Scenario: Batch input accepts device lists from a file or glob pattern
    Given the system is configured for cli batch operations
    When the feature is exercised
    Then batch input accepts device lists from a file or glob pattern
