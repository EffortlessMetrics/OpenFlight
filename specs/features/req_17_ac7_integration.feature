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
