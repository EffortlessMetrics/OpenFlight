@REQ-476 @product
Feature: DCS Falcon BMS Integration — Shared Memory Telemetry  @AC-476.1
  Scenario: Adapter reads from BMS FlightData shared memory mapping
    Given Falcon BMS is running and has created the FlightData shared memory region
    When the BMS adapter initialises
    Then the adapter SHALL successfully open and read the FlightData shared memory mapping  @AC-476.2
  Scenario: Shared memory fields are converted to BusSnapshot values
    Given the BMS adapter has an open shared memory handle
    When a new telemetry frame is available in shared memory
    Then the adapter SHALL convert the FlightData fields to a valid BusSnapshot and publish it  @AC-476.3
  Scenario: BMS disconnection is detected within 500ms
    Given the BMS adapter is running and receiving data
    When Falcon BMS closes the shared memory mapping
    Then the adapter SHALL detect the disconnection within 500ms and set the bus to a safe state  @AC-476.4
  Scenario: Adapter is only available on Windows where shared memory is supported
    Given the service is compiled without the windows-shared-memory feature on a non-Windows platform
    When the BMS adapter is requested
    Then the service SHALL report that the BMS adapter is unavailable on the current platform
