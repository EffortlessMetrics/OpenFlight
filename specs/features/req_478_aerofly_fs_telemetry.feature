@REQ-478 @product
Feature: AeroFly FS Telemetry — UDP Telemetry Processing  @AC-478.1
  Scenario: Adapter connects to AeroFly FS UDP data stream
    Given AeroFly FS is configured to broadcast UDP telemetry
    When the AeroFly FS adapter starts
    Then the adapter SHALL bind to the configured UDP port and receive telemetry packets  @AC-478.2
  Scenario: Aircraft state data is converted to BusSnapshot
    Given the AeroFly FS adapter is receiving UDP packets
    When a telemetry packet containing aircraft state is received
    Then the adapter SHALL convert the state data to a BusSnapshot and publish it to the bus  @AC-478.3
  Scenario: Aircraft type detection triggers appropriate profile selection
    Given a profile mapping is configured for AeroFly FS aircraft types
    When the adapter detects a change in the active aircraft type from telemetry
    Then the service SHALL select and activate the matching profile for that aircraft  @AC-478.4
  Scenario: Adapter handles AeroFly FS2 and AeroFly FS4 protocol variants
    Given the adapter is configured with protocol version set to auto-detect
    When packets from either AeroFly FS2 or AeroFly FS4 are received
    Then the adapter SHALL parse both protocol variants correctly without data loss
