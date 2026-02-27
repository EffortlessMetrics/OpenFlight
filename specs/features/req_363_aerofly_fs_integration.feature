@REQ-363 @product
Feature: AeroFly FS Integration  @AC-363.1
  Scenario: AeroFly UDP telemetry packets are parsed correctly
    Given the AeroFly adapter is running and receiving UDP telemetry
    When a valid AeroFly telemetry packet is received
    Then the packet SHALL be parsed without error  @AC-363.2
  Scenario: Attitude data is extracted and published to bus
    Given the AeroFly adapter is connected
    When a telemetry packet containing pitch, roll, and yaw data is processed
    Then the attitude data SHALL be published to the event bus  @AC-363.3
  Scenario: Connected and disconnected state is tracked per adapter lifecycle
    Given the AeroFly adapter has been started
    When AeroFly starts and stops sending telemetry
    Then the adapter SHALL transition between connected and disconnected states accordingly  @AC-363.4
  Scenario: AeroFly game manifest lists support tier and tested version
    Given the AeroFly adapter game manifest is loaded
    When the manifest is inspected
    Then it SHALL list the support tier and at least one tested version  @AC-363.5
  Scenario: Parser handles extended telemetry packets without overflow
    Given the AeroFly adapter is running
    When an extended telemetry packet with additional fields is received
    Then the parser SHALL process it without buffer overflow or data corruption  @AC-363.6
  Scenario: Fuzz target exists for the AeroFly UDP packet parser
    Given the AeroFly fuzz corpus is available
    When the fuzz target for the UDP packet parser is executed
    Then it SHALL not produce any panics or memory errors
