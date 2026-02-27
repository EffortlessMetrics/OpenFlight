@REQ-236 @product
Feature: AeroWinx PSX B747 simulator integrates via TCP text protocol  @AC-236.1
  Scenario: PSX TCP adapter connects on port 10747 by default
    Given AeroWinx PSX is running and listening on its default port
    When the PSX adapter initialises without explicit port configuration
    Then it SHALL attempt a TCP connection to port 10747 on the configured host  @AC-236.2
  Scenario: PSX key-value messages parsed and dispatched
    Given the PSX adapter has an active TCP connection
    When a message of the form "Qi=12345" is received
    Then the key "Qi" and value "12345" SHALL be parsed and dispatched as a bus event  @AC-236.3
  Scenario: PSX altitude heading and airspeed tracked in bus snapshot
    Given PSX is streaming telemetry messages
    When altitude, heading, and airspeed variables are received
    Then their latest values SHALL be reflected in the service bus state snapshot  @AC-236.4
  Scenario: Unknown PSX variables logged as warnings and ignored safely
    Given PSX sends a variable the adapter does not recognise
    When the unknown message is received
    Then the adapter SHALL log a warning and discard the message without error or panic  @AC-236.5
  Scenario: TCP disconnect from PSX triggers graceful transition to disconnected
    Given the PSX adapter has an established TCP connection
    When the TCP connection is lost
    Then the adapter SHALL transition to the disconnected state gracefully without crashing the service  @AC-236.6
  Scenario: PSX integration requires only standard TCP/IP with no special SDK
    Given a host with standard TCP/IP networking and no AeroWinx SDK installed
    When the PSX adapter attempts to connect to the simulator
    Then it SHALL establish communication using only the operating system TCP stack
