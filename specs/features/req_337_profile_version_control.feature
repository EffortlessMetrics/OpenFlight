@REQ-337 @product
Feature: Profile Version Control  @AC-337.1
  Scenario: Profile save creates a versioned backup
    Given a profile named "cessna172" exists on disk
    When the profile is saved with new settings
    Then the previous version SHALL be written as "cessna172.v1" alongside the active profile  @AC-337.2
  Scenario: Only the last 5 versions are retained
    Given a profile named "cessna172" already has 5 versioned backups (.v1 through .v5)
    When the profile is saved again
    Then the oldest backup SHALL be deleted and the remaining versions SHALL be renumbered  @AC-337.3
  Scenario: CLI lists available versions for a profile
    Given a profile named "cessna172" has 3 versioned backups
    When the user runs "flightctl profile versions cessna172"
    Then the command SHALL output a list of available versions with their index labels  @AC-337.4
  Scenario: CLI restores a specific version
    Given a profile named "cessna172" has a backup at version v3
    When the user runs "flightctl profile restore cessna172 v3"
    Then the active profile SHALL be replaced with the content of the v3 backup  @AC-337.5
  Scenario: Restore validates the target version before applying
    Given the user requests a restore of version v3 for profile "cessna172"
    When the restore command is executed
    Then the service SHALL validate the v3 backup passes schema validation before overwriting the active profile  @AC-337.6
  Scenario: Version metadata includes timestamp and optional description
    Given a profile save is triggered with an optional description "pre-flight tuning"
    When the versioned backup is created
    Then the backup metadata SHALL include the creation timestamp and the provided description
