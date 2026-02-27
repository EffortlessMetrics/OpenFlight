@REQ-377 @product
Feature: Profile Schema Versioning — Increment Schema Version on Breaking Changes

  @AC-377.1
  Scenario: Profile schema version is stored as semver in the root schema field
    Given a profile document
    When the profile is loaded
    Then the root `schema` field SHALL contain a valid semver string

  @AC-377.2
  Scenario: Breaking changes require a major version increment
    Given a profile schema change that removes or renames a required field
    When the schema version is updated
    Then the major version SHALL be incremented

  @AC-377.3
  Scenario: New optional fields increment the minor version
    Given a profile schema change that adds a new optional field
    When the schema version is updated
    Then the minor version SHALL be incremented and the major version left unchanged

  @AC-377.4
  Scenario: cargo xtask validate checks schema version consistency with CHANGELOG
    Given the schema version and CHANGELOG entries
    When `cargo xtask validate` is run
    Then it SHALL fail if the schema version is inconsistent with the CHANGELOG entries

  @AC-377.5
  Scenario: Migration paths exist for all version pairs since 1.0
    Given any two profile schema versions both at or after version 1.0
    When a migration is requested between those versions
    Then a migration path SHALL exist and complete without data loss

  @AC-377.6
  Scenario: Snapshot tests cover serialization and deserialization of each schema version
    Given all released schema versions
    When snapshot tests are run
    Then serialization and deserialization SHALL produce identical output for each version
