@REQ-441 @product
Feature: Profile Schema Migration — Migrate Older Profile Schemas to Current Version

  @AC-441.1
  Scenario: Profile parser detects schema version field and runs migration chain
    Given a profile file that declares schema_version: 1
    When the profile is loaded
    Then the parser SHALL detect the version and apply all applicable migrations in order

  @AC-441.2
  Scenario: V1 profiles are automatically upgraded to V2 on load
    Given a V1 profile file on disk
    When the profile system loads it
    Then the resulting in-memory profile SHALL conform to the V2 schema

  @AC-441.3
  Scenario: Migration preserves user axis configurations
    Given a V1 profile with custom axis curves and deadzones
    When the profile is migrated to V2
    Then all user axis configurations SHALL be present and unchanged in the migrated profile

  @AC-441.4
  Scenario: Migration is logged and original profile is backed up
    Given a V1 profile that requires migration
    When migration completes successfully
    Then a backup of the original profile SHALL exist on disk and a migration log entry SHALL be written
