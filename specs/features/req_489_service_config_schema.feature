@REQ-489 @product
Feature: Service Configuration Schema — Documented and Validated Config  @AC-489.1
  Scenario: Config schema is defined as a JSON Schema document
    Given the service configuration specification
    When the schema file is located
    Then it SHALL be a valid JSON Schema document  @AC-489.2
  Scenario: Unknown config keys produce warnings not errors
    Given a service config file containing an unrecognised key
    When the service loads the config
    Then a warning SHALL be emitted for the unknown key
    And the service SHALL continue loading normally  @AC-489.3
  Scenario: Schema is versioned and migration path is documented
    Given the JSON Schema document for service config
    When the schema version is inspected
    Then it SHALL contain a version identifier and documented migration path  @AC-489.4
  Scenario: flightctl config validate uses the schema
    Given a service config file
    When `flightctl config validate` is executed
    Then it SHALL validate the file against the JSON Schema and report any violations
