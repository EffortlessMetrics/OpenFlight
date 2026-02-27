@REQ-120 @product
Feature: Profile migration

  @AC-120.1
  Scenario: Profile v1 migrates to v2 with no data loss
    Given a profile document at schema version 1
    When the migration pipeline is applied
    Then the resulting profile SHALL be at schema version 2
    And no data from the v1 profile SHALL be lost during migration

  @AC-120.2
  Scenario: Migration preserves custom axis curves
    Given a profile document at schema version 1 with custom axis curve definitions
    When the migration pipeline is applied
    Then the migrated v2 profile SHALL contain all axis curve definitions from the source
    And curve control points SHALL be identical to the originals

  @AC-120.3
  Scenario: Migration preserves deadzone settings
    Given a profile document at schema version 1 with non-default deadzone values
    When the migration pipeline is applied
    Then the migrated v2 profile SHALL preserve all deadzone values
    And the deadzone ranges SHALL match the source profile exactly

  @AC-120.4
  Scenario: Invalid schema version rejected
    Given a profile document with an unrecognised schema version string
    When the migration pipeline is applied
    Then the pipeline SHALL return an InvalidSchemaVersion error
    And no migrated profile SHALL be produced

  @AC-120.5
  Scenario: Migration is idempotent
    Given a profile document at schema version 1
    When the migration pipeline is applied twice in succession
    Then the final profile SHALL be identical to the result of a single migration
    And the schema version SHALL remain at v2 after the second run

  @AC-120.6
  Scenario: Snapshot of migrated profile matches expected output
    Given a known v1 profile fixture
    When the migration pipeline is applied
    Then the resulting profile SHALL match the pre-approved v2 golden snapshot exactly
