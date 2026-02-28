@REQ-1050
Feature: Configuration Backup and Restore
  @AC-1050.1
  Scenario: Full configuration export creates a single archive file
    Given the system is configured for REQ-1050
    When the feature condition is met
    Then full configuration export creates a single archive file

  @AC-1050.2
  Scenario: Archive includes profiles, calibration data, and application settings
    Given the system is configured for REQ-1050
    When the feature condition is met
    Then archive includes profiles, calibration data, and application settings

  @AC-1050.3
  Scenario: Restore imports archive and validates contents before applying
    Given the system is configured for REQ-1050
    When the feature condition is met
    Then restore imports archive and validates contents before applying

  @AC-1050.4
  Scenario: Backup and restore operations are available via CLI and UI
    Given the system is configured for REQ-1050
    When the feature condition is met
    Then backup and restore operations are available via cli and ui
