Feature: DCS Export TCP Mode
  As a flight simulation enthusiast
  I want the DCS export adapter to support TCP as a transport
  So that telemetry is more reliable than UDP in lossy environments

  Background:
    Given the OpenFlight service is running
    And DCS TCP mode is enabled in service config on port 12345

  Scenario: DCS export script connects via TCP
    Given the DCS export Lua script is configured with TCP and port 12345
    When DCS World launches with the export script
    Then a TCP connection is established between DCS and the service

  Scenario: TCP connection loss triggers reconnect with backoff
    Given a TCP connection from DCS is established
    When the TCP connection is dropped
    Then the service attempts to reconnect
    And each retry uses exponential backoff up to a configured maximum interval

  Scenario: TCP mode togglable in service config
    Given the service config has "dcs_export.transport = udp"
    When the config is changed to "dcs_export.transport = tcp" and reloaded
    Then the DCS adapter switches to TCP mode
    And the previous UDP socket is closed
