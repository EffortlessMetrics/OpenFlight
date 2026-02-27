Feature: Service CLI JSON Output
  As a system integrator
  I want all flightctl commands to support structured JSON output
  So that I can integrate OpenFlight output into scripts and dashboards

  Background:
    Given the OpenFlight service is running

  Scenario: flightctl --json outputs valid JSON for all commands
    When flightctl --json status is run
    Then the output is valid JSON

  Scenario: JSON schema is documented for each command
    When the CLI reference documentation is opened
    Then each command includes a documented JSON output schema

  Scenario: JSON output is stable across patch versions
    Given JSON output is captured at version N
    When the same command is run at version N.0.1
    Then the JSON structure and field names are unchanged

  Scenario: Exit codes are independent of output format
    Given a command fails
    When flightctl --json runs the same failing command
    Then the exit code is the same as without --json
