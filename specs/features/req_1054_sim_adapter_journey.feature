@REQ-1054 @product @user-journey
Feature: Simulator adapter user journey
  As a pilot using OpenFlight with multiple simulators
  I want seamless simulator connection, disconnection, and aircraft detection
  So that my controls adapt automatically to the active simulator and aircraft

  @AC-1054.1
  Scenario: MSFS connection established via SimConnect
    Given the OpenFlight service is running and MSFS is installed
    When MSFS is launched and SimConnect becomes available
    Then the MSFS adapter SHALL establish a connection within 5 seconds
    And a "sim_connected" event SHALL be emitted with simulator identifier "MSFS"
    And the telemetry bus SHALL begin receiving MSFS flight data

  @AC-1054.2
  Scenario: X-Plane connection established via UDP
    Given the OpenFlight service is running and X-Plane 12 is installed
    When X-Plane is launched and begins broadcasting UDP data
    Then the X-Plane adapter SHALL detect the broadcast and connect
    And a "sim_connected" event SHALL be emitted with simulator identifier "X-Plane"
    And dataref subscriptions SHALL be established for required flight parameters

  @AC-1054.3
  Scenario: DCS connection established via Export.lua
    Given the OpenFlight service is running and DCS World is installed
    When DCS is launched with the Export.lua script configured
    Then the DCS adapter SHALL establish a telemetry connection
    And a "sim_connected" event SHALL be emitted with simulator identifier "DCS"
    And the adapter SHALL begin receiving cockpit state data

  @AC-1054.4
  Scenario: Simulator disconnect and reconnect preserves state
    Given the MSFS adapter is connected and axis data is flowing
    When MSFS is closed and then relaunched
    Then the adapter SHALL detect the disconnection within 5 seconds
    And a "sim_disconnected" event SHALL be emitted
    And when MSFS reconnects the adapter SHALL resume without manual intervention
    And the previously active profile SHALL be restored

  @AC-1054.5
  Scenario: Aircraft change triggers profile cascade update
    Given the MSFS adapter is connected with aircraft "Cessna 172"
    And a Cessna-specific profile and an F-18-specific profile both exist
    When the user switches aircraft to "F/A-18C" in the simulator
    Then the aircraft detector SHALL emit an "aircraft_changed" event within 500 ms
    And the profile cascade SHALL merge the F-18-specific overrides
    And the active axis configuration SHALL reflect the F-18 profile settings
