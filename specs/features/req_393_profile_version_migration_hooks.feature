@REQ-393 @product
Feature: Profile Version Migration Hooks — Allow Custom Migration Logic Per Version

  @AC-393.1
  Scenario: Migration hook is registered for a specific version pair
    Given the profile migration hook registry
    When a Rust function is registered for a (from, to) version pair
    Then it SHALL be invoked during migration between those versions

  @AC-393.2
  Scenario: Custom hook is called before the default field-rename migration
    Given a custom migration hook registered for a version pair
    When migration is triggered for that version pair
    Then the custom hook SHALL be called before the default field-rename migration runs

  @AC-393.3
  Scenario: Hook returning Err aborts migration leaving the profile unchanged
    Given a custom migration hook that returns Err
    When migration is triggered for that version pair
    Then the migration SHALL be aborted and the profile SHALL remain unchanged

  @AC-393.4
  Scenario: Hook has access to the raw TOML/JSON AST
    Given a custom migration hook registered for a version pair
    When the hook is invoked
    Then it SHALL receive the raw TOML/JSON AST for custom transformations

  @AC-393.5
  Scenario: Hook infrastructure is tested with a v1 to v2 migration
    Given a registered migration hook from schema v1 to v2
    When a v1 profile is migrated
    Then the hook SHALL be invoked and the output SHALL conform to v2 schema

  @AC-393.6
  Scenario: Missing hook falls through to default identity migration
    Given no migration hook is registered for a version pair
    When migration is triggered for that version pair
    Then the default identity migration SHALL be applied without error
