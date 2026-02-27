@REQ-553 @product
Feature: Profile Schema Migration — Profile schema version migrations should be automatic

  @AC-553.1
  Scenario: Profile loader detects schema version from file
    Given a profile file containing a schema_version field
    When the profile loader reads the file
    Then it SHALL correctly identify the schema version

  @AC-553.2
  Scenario: Profiles with older versions are migrated automatically
    Given a profile file with a schema version older than the current version
    When the profile loader reads the file
    Then it SHALL automatically apply all required migration steps

  @AC-553.3
  Scenario: Migration preserves user-specified values
    Given a profile with user-specified axis curve values in an older schema
    When the profile is migrated to the current schema version
    Then all user-specified values SHALL be present and unchanged in the migrated profile

  @AC-553.4
  Scenario: Migrated profile is saved back in new schema version
    Given a profile that has been successfully migrated
    When the service saves the profile
    Then the saved file SHALL contain the current schema version number
