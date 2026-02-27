@REQ-483 @product
Feature: Profile Validation Command — Offline Profile Validation  @AC-483.1
  Scenario: flightctl profile validate checks YAML schema and field values
    Given a profile YAML file on disk
    When `flightctl profile validate <path>` is executed
    Then the command SHALL check the file against the profile schema and validate all field values  @AC-483.2
  Scenario: Validation reports all errors not just the first
    Given a profile YAML file containing multiple schema violations
    When `flightctl profile validate <path>` is executed
    Then the output SHALL list all validation errors found rather than stopping at the first  @AC-483.3
  Scenario: Valid profiles exit 0 and invalid profiles exit 1 with error summary
    Given a valid profile file and a separate invalid profile file
    When `flightctl profile validate` is run on each file
    Then the valid file run SHALL exit with code 0 and the invalid file run SHALL exit with code 1 with an error summary  @AC-483.4
  Scenario: Validation includes checking referenced device manifests exist
    Given a profile that references a device manifest by identifier
    When `flightctl profile validate <path>` is executed
    Then the validator SHALL confirm the referenced device manifest exists and report an error if it does not
