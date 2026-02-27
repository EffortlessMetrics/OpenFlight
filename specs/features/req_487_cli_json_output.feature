@REQ-487 @product
Feature: CLI JSON Output Mode — Machine-Readable CLI Output  @AC-487.1
  Scenario: All flightctl commands support --json flag
    Given the flightctl CLI is installed
    When any flightctl command is invoked with the --json flag
    Then the command SHALL accept the flag without error  @AC-487.2
  Scenario: JSON output includes all human-readable fields in structured form
    Given a flightctl command that produces human-readable output
    When the command is invoked with --json
    Then the JSON output SHALL contain all fields present in the human-readable output  @AC-487.3
  Scenario: Exit codes are consistent across JSON and human output modes
    Given a flightctl command invoked with --json
    When the command succeeds
    Then the exit code SHALL be 0
    And when the command fails the exit code SHALL be non-zero  @AC-487.4
  Scenario: JSON format is stable and versioned
    Given a flightctl command invoked with --json
    When the JSON output is inspected
    Then it SHALL contain a schema_version field identifying the output format version
