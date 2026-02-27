@REQ-502 @product
Feature: Profile Backup and Restore — Automatic Versioned Profile Backups  @AC-502.1
  Scenario: Profile backup is created automatically before any modification
    Given a profile exists on disk
    When a profile modification command is issued via flightctl
    Then a backup copy SHALL be created in the backup directory before the change is applied  @AC-502.2
  Scenario: Backup is stored in a versioned backup directory
    Given a profile has been modified multiple times
    When the backup directory is listed
    Then each backup SHALL be stored with a timestamp-versioned filename  @AC-502.3
  Scenario: flightctl profile restore lists and restores backups
    Given at least two backups exist for the active profile
    When `flightctl profile restore` is invoked
    Then the command SHALL list available backups and restore the selected one  @AC-502.4
  Scenario: Maximum backup count is configurable
    Given the service is configured with a maximum backup count of 5
    When a sixth backup would be created
    Then the oldest backup SHALL be deleted to maintain the configured maximum
