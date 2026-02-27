@REQ-191 @product
Feature: Profiles can be exported to and imported from portable formats  @AC-191.1
  Scenario: Profile exported as TOML with all axis configs and device mappings
    Given an active profile with axis configurations and device mappings
    When the profile is exported
    Then a TOML file SHALL be produced containing all axis configurations and device mappings  @AC-191.2
  Scenario: Exported profile imports cleanly on a different machine
    Given a TOML profile file exported from one machine
    When the file is imported on a different machine with compatible hardware
    Then the profile SHALL be applied without errors and all settings SHALL be preserved  @AC-191.3
  Scenario: Import validates schema before applying profile
    Given a TOML profile file to be imported
    When the import command is run
    Then schema validation SHALL complete successfully before the profile is applied  @AC-191.4
  Scenario: Unknown fields generate warnings not errors
    Given a TOML profile file containing fields not present in the current schema
    When the profile is imported
    Then a warning SHALL be emitted for each unknown field and the import SHALL succeed  @AC-191.5
  Scenario: Export includes version and schema identifiers
    Given an active profile
    When the profile is exported to TOML
    Then the exported file SHALL contain version and schema identifier fields  @AC-191.6
  Scenario: CLI export command works end to end
    Given the OpenFlight service is running with an active profile
    When the user runs flightctl profile export --file out.toml
    Then a valid TOML profile file SHALL be written to out.toml and confirmed by the CLI
