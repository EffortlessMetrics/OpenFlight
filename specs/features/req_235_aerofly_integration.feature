@REQ-235 @product
Feature: Aerofly FS 2/4 adapter provides telemetry and control support  @AC-235.1
  Scenario: Aerofly FS UDP interface polled for aircraft state
    Given Aerofly FS is running and broadcasting on the configured UDP address
    When the Aerofly adapter is active
    Then the adapter SHALL poll the UDP interface and receive aircraft state packets  @AC-235.2
  Scenario: Altitude airspeed and attitude decoded from Aerofly JSON packets
    Given a valid Aerofly UDP packet is received
    When the packet is decoded
    Then altitude, airspeed, and attitude values SHALL be correctly extracted and made available on the bus  @AC-235.3
  Scenario: Aircraft type extracted from Aerofly telemetry payload
    Given a telemetry packet containing aircraft identification data
    When the packet is processed
    Then the aircraft type string SHALL be parsed and published as the active aircraft identifier  @AC-235.4
  Scenario: Control inputs sent to Aerofly via UDP commands
    Given the Aerofly adapter is connected and the service has axis values to send
    When the RT spine produces a new set of axis outputs
    Then the adapter SHALL encode and transmit the control input via UDP command packets  @AC-235.5
  Scenario: Aerofly FS 2 and FS 4 both supported by same adapter
    Given either Aerofly FS 2 or Aerofly FS 4 is running
    When the adapter connects
    Then it SHALL communicate correctly with both simulator versions without separate configuration  @AC-235.6
  Scenario: Network address and port configurable in service config
    Given a service configuration file specifying a custom Aerofly UDP address and port
    When the Aerofly adapter initialises
    Then it SHALL bind to the address and port from the configuration rather than the defaults
