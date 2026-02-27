@REQ-307 @product
Feature: Falcon BMS Integration

  @AC-307.1
  Scenario: Service reads Falcon BMS shared memory segment
    Given Falcon BMS is running and has created the "BMS-Data" shared memory segment
    When the service starts with a Falcon BMS profile
    Then the service SHALL open and read the "BMS-Data" shared memory segment for flight data

  @AC-307.2
  Scenario: Shared memory maps BMS flight data struct
    Given the service has opened the BMS-Data shared memory segment
    When the segment is mapped
    Then the service SHALL extract airspeed, altitude, heading, and G-force from the mapped BMS flight data struct

  @AC-307.3
  Scenario: Integration uses polling at 30Hz configurable rate
    Given the service is connected to BMS shared memory
    When the service is running normally
    Then the service SHALL poll the shared memory segment at 30Hz by default and SHALL respect a configurable polling rate

  @AC-307.4
  Scenario: BMS session start and stop is detected automatically
    Given the service is monitoring Falcon BMS
    When a BMS session starts or stops
    Then the service SHALL automatically detect the session state change and update its connection status accordingly

  @AC-307.5
  Scenario: Profile switching based on vehicle type in BMS
    Given the service is reading BMS shared memory
    When the vehicle type reported in the BMS data changes
    Then the service SHALL switch to the profile configured for the new vehicle type

  @AC-307.6
  Scenario: Integration test uses mock shared memory segment
    Given a mock "BMS-Data" shared memory segment is created with known flight data values
    When the integration test runs the BMS adapter against the mock segment
    Then the adapter SHALL read the expected airspeed, altitude, heading, and G-force values from the mock segment
