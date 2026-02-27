Feature: MSFS WASM Module Integration
  As a flight simulation enthusiast
  I want the service to integrate with an MSFS WASM gauge module
  So that cockpit variables can be read and written directly

  Background:
    Given the OpenFlight service is running
    And MSFS is running with the OpenFlight WASM module installed

  Scenario: WASM module provides direct cockpit variable access
    Given the WASM module is connected via named pipe
    When the service requests the value of cockpit variable "FLAPS_HANDLE_INDEX"
    Then the value is returned without going through SimConnect

  Scenario: Connection state with WASM module is monitored
    Given the WASM module connection is established
    When the WASM module disconnects unexpectedly
    Then the connection state is updated to "disconnected"
    And a reconnect attempt is scheduled

  Scenario: WASM module installable via packaging
    When the OpenFlight installer runs
    Then the WASM gauge module is copied to the MSFS Community folder
    And the module is detectable by the service on next MSFS start
