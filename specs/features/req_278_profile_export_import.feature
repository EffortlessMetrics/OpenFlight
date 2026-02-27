@REQ-278 @product
Feature: Profile export and import preserves all axis configs with schema migration and round-trip fidelity  @AC-278.1
  Scenario: Profile can be exported to YAML file via CLI
    Given the service has an active profile loaded
    When the user runs the CLI export command specifying an output file path
    Then a YAML file SHALL be written to that path containing the profile data  @AC-278.2
  Scenario: Exported file can be imported back without loss
    Given a profile has been exported to a YAML file
    When the user runs the CLI import command with that file
    Then the imported profile SHALL contain all the same axis configs and settings as the original  @AC-278.3
  Scenario: Import validates profile schema before applying
    Given a YAML file that does not conform to the profile schema
    When the user runs the CLI import command with that file
    Then the import SHALL fail with a schema validation error and the current profile SHALL remain unchanged  @AC-278.4
  Scenario: Exported profile includes all axis configs and curve points
    Given a profile with multiple axes each having custom curves and deadzones
    When the profile is exported to YAML
    Then the exported file SHALL include the full curve point lists and deadzone values for every axis  @AC-278.5
  Scenario: Import handles schema version migration automatically
    Given a YAML profile file written against an older schema version
    When the user imports that file
    Then the service SHALL migrate the profile to the current schema version before applying it  @AC-278.6
  Scenario: Round-trip export and import produces identical profile
    Given a profile is exported and then immediately re-imported
    When the re-imported profile is compared to the original in-memory profile
    Then the two profiles SHALL be structurally identical with no data loss or modification
