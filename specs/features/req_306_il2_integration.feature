@REQ-306 @product
Feature: IL-2 Sturmovik Integration

  @AC-306.1
  Scenario: Service connects to IL-2 via UDP export on port 21001
    Given IL-2 Sturmovik is configured to send UDP telemetry
    When the service starts with an IL-2 profile
    Then the service SHALL listen for IL-2 UDP export packets on port 21001

  @AC-306.2
  Scenario: IL-2 export protocol provides airspeed altitude heading pitch and roll
    Given the service is receiving IL-2 UDP packets
    When a telemetry packet is parsed
    Then the service SHALL extract airspeed, altitude, heading, pitch, and roll from the packet payload

  @AC-306.3
  Scenario: Protocol magic bytes are validated on every packet
    Given the service is listening on the IL-2 UDP port
    When a UDP packet arrives
    Then the service SHALL validate the protocol magic bytes before processing any payload data and discard packets that fail validation

  @AC-306.4
  Scenario: Connection loss triggers stale snapshot after 1 second
    Given the service is receiving IL-2 telemetry packets
    When no valid packet arrives for more than 1 second
    Then the service SHALL mark the telemetry snapshot as stale and stop updating axis values from IL-2 data

  @AC-306.5
  Scenario: Aircraft type from IL-2 triggers profile matching
    Given the service is receiving IL-2 telemetry
    When the aircraft identifier in the telemetry changes
    Then the service SHALL attempt to match the new aircraft type to a configured profile and activate it

  @AC-306.6
  Scenario: Integration tests replay captured IL-2 UDP packets
    Given a set of captured IL-2 UDP packet recordings
    When the integration test replays those packets to the service listener
    Then the service SHALL parse each packet correctly and produce the expected telemetry field values
