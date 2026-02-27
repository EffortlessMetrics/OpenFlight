@REQ-222 @product
Feature: MSFS SimConnect adapter provides full bidirectional control and telemetry  @AC-222.1
  Scenario: Axis inputs written to MSFS via SimConnect within 20ms
    Given the MSFS SimConnect adapter is connected and an axis input event arrives
    When the axis value changes at the HID layer
    Then the value SHALL be written to MSFS via SimConnect within 20 ms  @AC-222.2
  Scenario: MSFS telemetry published to bus
    Given the MSFS adapter is connected to a running MSFS session with an airborne aircraft
    When telemetry packets are received from SimConnect
    Then altitude, airspeed, and attitude data SHALL be published to the telemetry bus  @AC-222.3
  Scenario: MSFS aircraft ICAO type used for auto-profile selection
    Given the MSFS adapter is connected and an aircraft is loaded in the simulator
    When the aircraft ICAO type is received from SimConnect
    Then the ICAO type SHALL be published to the bus and used to trigger auto-profile selection  @AC-222.4
  Scenario: SimConnect reconnect handled after MSFS restart
    Given the MSFS adapter has an active SimConnect connection and MSFS is restarted
    When MSFS becomes available again after the restart
    Then the adapter SHALL reconnect to SimConnect automatically without requiring flightd restart  @AC-222.5
  Scenario: MSFS pause state reflected in adapter status
    Given the MSFS adapter is connected to a running session
    When MSFS enters the paused state
    Then the adapter status SHALL reflect paused and publish the state change to the bus  @AC-222.6
  Scenario: MSFS 2020 and MSFS 2024 both supported with same adapter
    Given the same adapter and configuration with no version-specific overrides
    When connected to either MSFS 2020 or MSFS 2024
    Then the adapter SHALL operate correctly with both simulator versions without code changes
