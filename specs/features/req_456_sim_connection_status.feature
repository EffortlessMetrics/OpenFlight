@REQ-456 @product
Feature: Sim Connection Status Broadcasting — Broadcast Simulator Connection Events on flight-bus

  @AC-456.1
  Scenario: SimConnected event is published when adapter connects
    Given a service with no active simulator connection
    When the MSFS SimConnect adapter establishes a connection
    Then a SimConnected event SHALL be published on the flight-bus

  @AC-456.2
  Scenario: SimDisconnected event is published when adapter disconnects
    Given a service with an active simulator connection
    When the simulator closes or the adapter loses the connection
    Then a SimDisconnected event SHALL be published on the flight-bus

  @AC-456.3
  Scenario: Connection status includes simulator type and version
    Given a SimConnected event has been published
    When a subscriber reads the event payload
    Then the payload SHALL include the simulator type (e.g. MSFS2024) and version string

  @AC-456.4
  Scenario: Connection status is queryable via IPC at any time
    Given a service in any connection state
    When a GetSimStatus IPC request is issued
    Then the response SHALL reflect the current connection state, simulator type, and version
