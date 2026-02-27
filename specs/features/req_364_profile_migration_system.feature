@REQ-364 @product
Feature: Profile Migration System  @AC-364.1
  Scenario: Profiles with schema version below current are detected on load
    Given a profile file with a schema version older than the current version
    When the profile is loaded by the service
    Then the service SHALL detect that migration is required  @AC-364.2
  Scenario: Migration is applied step-by-step through each schema version
    Given a profile at schema version N that is two versions behind current
    When the migration system processes the profile
    Then it SHALL apply migrations sequentially through each intermediate version  @AC-364.3
  Scenario: Migration failures produce a specific error with source version and failure location
    Given a profile that cannot be migrated due to invalid data
    When the migration system attempts to process it
    Then it SHALL return an error identifying the source schema version and the step that failed  @AC-364.4
  Scenario: Migrated profile is re-validated against the current schema
    Given a profile has been successfully migrated to the current schema version
    When migration completes
    Then the migrated profile SHALL be validated against the current schema before use  @AC-364.5
  Scenario: Original profile file is backed up before migration
    Given a profile that requires migration exists on disk
    When migration begins
    Then the original file SHALL be backed up before any changes are written  @AC-364.6
  Scenario: Golden file tests cover migrations from version 1 through current
    Given the golden file test suite for profile migrations
    When the tests are executed
    Then migrations from schema version 1 through the current version SHALL all produce expected output
