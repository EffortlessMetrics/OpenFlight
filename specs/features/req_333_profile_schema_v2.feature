@REQ-333 @product
Feature: Profile Schema v2  @AC-333.1
  Scenario: Schema v2 adds device-level config support
    Given a v2 profile YAML
    When the profile contains a devices section
    Then the schema SHALL validate device-level configuration entries without error  @AC-333.2
  Scenario: v2 adds per-axis filter chain configuration
    Given a v2 profile with axes defined
    When an axis entry includes a filter_chain field
    Then the schema SHALL validate filter chain entries (e.g., low-pass, notch) without error  @AC-333.3
  Scenario: v2 adds inheritance parent field
    Given a v2 profile YAML
    When a parent field specifying another profile name is present
    Then the schema SHALL accept the parent field and record the inheritance relationship  @AC-333.4
  Scenario: v1 profiles are automatically migrated to v2 on load
    Given a v1 profile file on disk
    When the service loads the profile
    Then the profile SHALL be transparently migrated to v2 in memory without user intervention  @AC-333.5
  Scenario: v2 validation checks new fields for correctness
    Given a v2 profile with an invalid filter_chain type
    When validation is run
    Then validation SHALL reject the profile with a descriptive error  @AC-333.6
  Scenario: Schema version is embedded in profile YAML header
    Given a profile saved by the service
    When the file is inspected
    Then the first lines SHALL contain a schema_version: 2 field
