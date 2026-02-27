@REQ-288 @product
Feature: Profile schema migrations with ordered version steps, setting preservation, error messages, and logging  @AC-288.1
  Scenario: Profile v1 schema is automatically upgraded to v2
    Given a stored profile file containing a v1 schema document
    When the service loads that profile
    Then the profile SHALL be automatically migrated to the v2 schema without user intervention  @AC-288.2
  Scenario: Migration preserves all axis settings
    Given a v1 profile with axis curves, deadzones, and trim values defined
    When the profile is migrated to v2
    Then all axis settings from the original profile SHALL be present and unchanged in the migrated profile  @AC-288.3
  Scenario: Migration failures produce descriptive error messages
    Given a profile file that is structurally invalid and cannot be migrated
    When the service attempts migration
    Then the operation SHALL fail with an error message that identifies the field or constraint that caused the failure  @AC-288.4
  Scenario: Old schema version is recorded in migrated profile
    Given a profile that has been migrated from v1 to v2
    When the migrated profile metadata is inspected
    Then it SHALL contain a field recording that the original schema version was v1  @AC-288.5
  Scenario: Migrations are applied in version order
    Given a v1 profile and migration steps for v1 to v2 and v2 to v3
    When the profile is loaded by a service expecting v3
    Then the v1 to v2 migration SHALL be applied first followed by the v2 to v3 migration  @AC-288.6
  Scenario: Migration results are logged with source and target version
    Given the service migrates a profile from one schema version to another
    When the migration completes
    Then a structured log entry SHALL be written containing the source schema version and the target schema version
