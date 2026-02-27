Feature: Service Config Hot Validation
  As a flight simulation enthusiast
  I want the service to validate config changes before applying them
  So that invalid configurations are rejected without disrupting active sessions

  Background:
    Given the OpenFlight service is running with a valid configuration

  Scenario: Config validation runs synchronously before hot-reload
    When the operator issues a hot-reload command with a new config file
    Then the service validates the new config before applying it
    And the reload response indicates whether validation passed or failed

  Scenario: Invalid config is rejected and old config stays active
    Given a config file containing an invalid axis curve definition
    When the operator triggers a hot-reload with that config
    Then the service rejects the new config
    And the service continues operating with the previously active config

  Scenario: Validation result is returned in CLI response
    When the operator runs "flightctl config reload /path/to/config.toml"
    Then the CLI prints the validation result
    And the exit code is non-zero if validation failed

  Scenario: Validation errors include file path and line number
    Given a config file with a syntax error on line 42
    When validation runs on that config
    Then the validation error message includes the file path and line number 42
