@REQ-465 @product
Feature: War Thunder Telemetry Integration — UDP Telemetry to BusSnapshot  @AC-465.1
  Scenario: Adapter connects to War Thunder telemetry port 8111
    Given the War Thunder adapter is enabled in config
    When the service starts
    Then the adapter SHALL attempt to connect to localhost port 8111 for telemetry  @AC-465.2
  Scenario: Telemetry JSON is parsed and published as BusSnapshot
    Given the War Thunder adapter is connected
    When a valid telemetry JSON payload is received
    Then the adapter SHALL parse the payload and publish a BusSnapshot on the flight-bus  @AC-465.3
  Scenario: Aircraft type from telemetry triggers profile selection
    Given the War Thunder adapter is running and auto-profile selection is enabled
    When telemetry indicates an aircraft type change
    Then the profile manager SHALL select the matching profile for that aircraft type  @AC-465.4
  Scenario: Parse errors are counted and logged without crashing
    Given the War Thunder adapter is running
    When a malformed or incomplete telemetry JSON payload is received
    Then the adapter SHALL increment the parse error counter, log the error, and continue running
