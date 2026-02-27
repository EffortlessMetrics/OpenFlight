@REQ-361 @product
Feature: IL-2 Sturmovik Integration  @AC-361.1
  Scenario: IL-2 telemetry packets are parsed from UDP port 29373
    Given the IL-2 adapter is configured with default settings
    When the adapter binds to the telemetry port
    Then it SHALL listen on UDP port 29373  @AC-361.2
  Scenario: Flight state is published to the bus
    Given the IL-2 adapter is connected and receiving telemetry
    When a telemetry packet containing position, velocity, and attitude is received
    Then the flight state SHALL be published to the event bus  @AC-361.3
  Scenario: Aircraft type is detected from IL-2 telemetry headers
    Given the IL-2 adapter is receiving telemetry packets
    When a packet with an aircraft type field in the header is processed
    Then the detected aircraft type SHALL be published to the bus  @AC-361.4
  Scenario: Reconnection is automatic after IL-2 restart within 10 seconds
    Given the IL-2 adapter was connected and IL-2 has been restarted
    When IL-2 resumes sending telemetry
    Then the adapter SHALL reconnect automatically within 10 seconds  @AC-361.5
  Scenario: IL-2 game manifest lists supported versions
    Given the IL-2 adapter game manifest is loaded
    When the manifest is inspected for supported versions
    Then it SHALL list version 1.9 and all later supported versions  @AC-361.6
  Scenario: Telemetry parser handles partial or malformed packets without panic
    Given the IL-2 adapter is running
    When a partial or malformed UDP packet is received by the parser
    Then the parser SHALL handle the packet gracefully without panicking
