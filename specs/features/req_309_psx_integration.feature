@REQ-309 @product
Feature: PSX Professional SimTool X Integration

  @AC-309.1
  Scenario: Service connects to PSX via TCP socket on port 10747
    Given PSX is running and listening for client connections
    When the service starts with a PSX profile
    Then the service SHALL establish a TCP connection to PSX on port 10747

  @AC-309.2
  Scenario: PSX sends NMEA-like key=value lines
    Given the service is connected to PSX via TCP
    When PSX transmits simulation data
    Then the service SHALL receive and process NMEA-like key=value text lines from PSX

  @AC-309.3
  Scenario: Line parser extracts speed altitude heading gear and flap state
    Given the service is receiving PSX key=value lines
    When a data line is parsed
    Then the service SHALL extract speed, altitude, heading, gear state, and flap state from the parsed key=value pairs

  @AC-309.4
  Scenario: Connection drop is detected within 2 seconds
    Given the service has an active TCP connection to PSX
    When the connection drops or PSX stops sending data
    Then the service SHALL detect the connection loss within 2 seconds and update its connection state accordingly

  @AC-309.5
  Scenario: Reconnection is automatic with 5-second backoff
    Given the service has detected a dropped PSX TCP connection
    When the reconnection timer fires
    Then the service SHALL automatically attempt to reconnect to PSX with a 5-second backoff between attempts

  @AC-309.6
  Scenario: Integration test uses mock TCP server
    Given a mock TCP server is started on port 10747 sending PSX-format key=value lines
    When the integration test runs the PSX adapter against the mock server
    Then the adapter SHALL parse all lines correctly and produce the expected speed, altitude, heading, gear, and flap values
