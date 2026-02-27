@REQ-304 @product
Feature: DCS Export Script Integration

  @AC-304.1
  Scenario: DCS integration uses Lua export script with Export.lua hooks
    Given a DCS World installation with the service export script deployed
    When DCS World starts a mission
    Then the Export.lua hooks SHALL be invoked by DCS and relay aircraft data to the service

  @AC-304.2
  Scenario: Export.lua is generated from profile configuration
    Given an active profile with DCS export settings defined
    When the user requests export script generation
    Then the service SHALL produce a valid Export.lua file derived from the profile configuration

  @AC-304.3
  Scenario: Script sends aircraft data via UDP on port 27101
    Given the Export.lua script is active during a DCS mission
    When the export hook fires
    Then the script SHALL transmit aircraft state data as UDP datagrams to localhost port 27101

  @AC-304.4
  Scenario: Service receives and parses DCS telemetry packets
    Given the service is listening on UDP port 27101
    When a telemetry packet arrives from the DCS Export.lua script
    Then the service SHALL successfully parse the packet and extract aircraft state fields

  @AC-304.5
  Scenario: Connection loss is detected within 1 second
    Given the service is receiving DCS telemetry packets
    When no packet is received for more than 1 second
    Then the service SHALL transition to a disconnected state and emit a connection-loss event

  @AC-304.6
  Scenario: Re-connection happens automatically when DCS reconnects
    Given the service has detected a DCS connection loss
    When DCS starts sending telemetry packets again
    Then the service SHALL automatically resume processing without requiring manual intervention
