@REQ-221 @product
Feature: DCS World adapter provides full telemetry and control injection  @AC-221.1
  Scenario: DCS Export.lua configured automatically on first sim connection
    Given OpenFlight is started with DCS World installed but Export.lua not yet configured
    When the first connection to DCS is initiated
    Then the Export.lua script SHALL be deployed to the DCS Scripts directory automatically  @AC-221.2
  Scenario: Altitude airspeed AoA and gear state telemetry received from DCS
    Given the DCS adapter is connected to a running DCS World session
    When the aircraft is in flight
    Then altitude, airspeed, angle of attack, and gear state SHALL be published to the telemetry bus  @AC-221.3
  Scenario: FFB forces from DCS translated to OpenFlight FFB pipeline
    Given the DCS adapter is active and a FFB-capable device is connected
    When DCS transmits force feedback data via the export interface
    Then the forces SHALL be translated and submitted to the OpenFlight FFB pipeline  @AC-221.4
  Scenario: Module name detected from DCS telemetry
    Given the DCS adapter is receiving telemetry from a running session
    When an aircraft module is loaded in DCS
    Then the module name SHALL be extracted from telemetry and published as the aircraft type on the bus  @AC-221.5
  Scenario: Reconnection after DCS crash without service restart
    Given the DCS adapter has an active connection that is severed by a DCS crash
    When DCS restarts and becomes available again
    Then the adapter SHALL reconnect automatically without requiring flightd to be restarted  @AC-221.6
  Scenario: DCS adapter validated across multiple module families
    Given the DCS adapter running integration tests with available module stubs
    When test scenarios for F-16, F/A-18, and A-10 module families are executed
    Then all module families SHALL produce valid telemetry and accept control injection without error
