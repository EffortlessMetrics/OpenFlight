@REQ-227 @product
Feature: Falcon BMS shared memory adapter provides full avionics state access  @AC-227.1
  Scenario: BMS-Data shared memory segment opened at service start
    Given Falcon BMS is installed and has created the BMS-Data shared memory segment
    When the OpenFlight service starts
    Then the adapter SHALL open the BMS-Data segment and begin reading avionics data  @AC-227.2
  Scenario: F-16 avionics data extracted from shared memory
    Given the BMS-Data shared memory segment is open
    When the adapter reads avionics data
    Then altitude, airspeed, angle of attack, and G-force SHALL be extracted and published to the bus  @AC-227.3
  Scenario: Cockpit switch states decoded from bitfields
    Given the BMS-Data shared memory segment contains cockpit switch bitfields
    When the adapter decodes the bitfields
    Then gear, flaps, and master arm states SHALL be correctly represented in the telemetry snapshot  @AC-227.4
  Scenario: BMS version number validated at connection
    Given the BMS-Data shared memory segment is available
    When the adapter opens the segment
    Then it SHALL read the BMS version field and log a warning if the version is outside the supported range  @AC-227.5
  Scenario: Missing shared memory segment handled gracefully
    Given Falcon BMS is not running and the shared memory segment does not exist
    When the adapter attempts to open BMS-Data
    Then the adapter SHALL enter disconnected state and not crash or block service startup  @AC-227.6
  Scenario: BMS paused state reflected in telemetry staleness
    Given the BMS adapter is connected and reading data
    When Falcon BMS enters a paused state
    Then the adapter SHALL mark the telemetry snapshot as stale while BMS remains paused
