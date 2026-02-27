@REQ-223 @product
Feature: X-Plane UDP adapter provides full telemetry and axis output  @AC-223.1
  Scenario: X-Plane dataref telemetry received over UDP on default port 49000
    Given the X-Plane UDP adapter configured with the default port 49000
    When X-Plane is running and transmitting dataref packets
    Then telemetry data SHALL be received and parsed correctly from UDP port 49000  @AC-223.2
  Scenario: Axis inputs sent to X-Plane via DREF or CMND UDP protocol
    Given the X-Plane adapter is active and an axis input event is received
    When the axis value changes
    Then the value SHALL be transmitted to X-Plane using the DREF or CMND UDP protocol  @AC-223.3
  Scenario: Aircraft type detected from X-Plane ACFT dataref
    Given the X-Plane adapter is receiving telemetry from a running session
    When the ACFT dataref value is received
    Then the aircraft type SHALL be extracted and published to the bus for auto-profile selection  @AC-223.4
  Scenario: X-Plane 11 and X-Plane 12 both supported without config changes
    Given the X-Plane adapter configured without version-specific settings
    When connected to either X-Plane 11 or X-Plane 12
    Then the adapter SHALL operate correctly with both versions without configuration changes  @AC-223.5
  Scenario: X-Plane UDP reconnect within 2 seconds of connection loss
    Given the X-Plane adapter has an active UDP session that is then lost
    When X-Plane becomes reachable again on the network
    Then the adapter SHALL re-establish the UDP connection within 2 seconds  @AC-223.6
  Scenario: X-Plane network address configurable in service config file
    Given a service config file specifying a custom X-Plane IP address and port
    When the service is started with that config file
    Then the X-Plane adapter SHALL connect to the address and port specified in the config file
