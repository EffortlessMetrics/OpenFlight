@REQ-17
Feature: Ace Combat 7 experimental bridge integration

  @AC-17.1
  Scenario: Protocol validates AC7 telemetry JSON
    Given an AC7 bridge payload using schema "flight.ac7.telemetry/1"
    When Flight Hub parses the payload
    Then protocol validation SHALL accept in-range fields
    And protocol validation SHALL reject out-of-range fields

  @AC-17.2
  Scenario: Telemetry adapter publishes Ace Combat 7 snapshots
    Given an AC7 telemetry adapter listening on localhost UDP
    When a valid bridge packet is sent to the adapter
    Then the adapter SHALL emit a BusSnapshot with sim "AceCombat7"
    And control input fields SHALL map to normalized bus controls

  @AC-17.3
  Scenario: Input installer manages Input.ini safely
    Given an existing AC7 Input.ini with user content
    When Flight Hub installs a managed AC7 profile
    Then EnableJoystick SHALL be set to True
    And exactly one Flight Hub managed block SHALL exist
    And a backup SHALL be created when backup mode is enabled

  @AC-17.4
  Scenario: Protocol rejects unsupported schema and invalid JSON
    Given an AC7 bridge payload using schema "flight.ac7.telemetry/1"
    When Flight Hub parses the payload
    Then protocol validation SHALL reject malformed inputs

  @AC-17.5
  Scenario: Protocol JSON round-trip preserves values
    Given an AC7 bridge payload using schema "flight.ac7.telemetry/1"
    When Flight Hub parses the payload
    Then the payload SHALL round-trip through JSON

  @AC-17.6
  Scenario: Protocol property bounds cover control fields
    Given an AC7 bridge payload using schema "flight.ac7.telemetry/1"
    Then control field ranges SHALL be enforced

  @AC-17.7
  Scenario: Telemetry adapter lifecycle is stable
    Given an AC7 telemetry adapter listening on localhost UDP
    Then the adapter SHALL start and stop cleanly

  @AC-17.8
  Scenario: Telemetry adapter sets validity flags correctly
    Given an AC7 telemetry adapter listening on localhost UDP
    Then snapshot validity flags SHALL reflect packet completeness

  @AC-17.9
  Scenario: Input profile validation rejects unsafe names
    Given an AC7 input profile
    Then validation SHALL reject names containing quotes

  @AC-17.10
  Scenario: Managed Input.ini block is idempotent and reversible
    Given an existing AC7 Input.ini with user content
    Then applying the managed block SHALL be idempotent

  @AC-17.11
  Scenario: Input profile property bounds cover numeric ranges
    Given an AC7 input profile
    Then numeric ranges SHALL be enforced
