Feature: X-Plane Plugin Protocol Support
  As a flight simulation enthusiast
  I want the X-Plane adapter to support the native plugin protocol
  So that I can use DataRef-based data exchange as an alternative to UDP

  Background:
    Given the OpenFlight service is running
    And X-Plane is running with the ExtPlane plugin installed

  Scenario: ExtPlane plugin protocol is supported as an alternative to UDP
    Given the X-Plane adapter config sets protocol to "extplane"
    When the adapter initialises
    Then it connects using the ExtPlane TCP protocol instead of UDP

  Scenario: Plugin protocol connection is established on port 51000
    Given the ExtPlane plugin is listening on its default port
    When the adapter connects
    Then the TCP connection is made to port 51000 on the X-Plane host

  Scenario: Plugin protocol supports DataRef subscription and unsubscription
    Given the adapter is connected via the ExtPlane protocol
    When the adapter subscribes to the DataRef "sim/cockpit/autopilot/altitude"
    Then X-Plane begins sending updates for that DataRef
    When the adapter unsubscribes from the DataRef
    Then X-Plane stops sending updates for that DataRef

  Scenario: Plugin protocol uses TCP with line-oriented framing
    Given the adapter is connected via the ExtPlane protocol
    When the adapter sends a command
    Then the command is sent as a newline-terminated ASCII line over the TCP connection
