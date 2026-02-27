@REQ-425 @product
Feature: Sim Adapter Status Polling — Poll Adapter Status from CLI

  @AC-425.1
  Scenario: flightctl sim status shows connection state for all sim adapters
    Given the service is running with one or more sim adapters configured
    When `flightctl sim status` is executed
    Then the connection state of every configured adapter SHALL be displayed

  @AC-425.2
  Scenario: Status includes connected, last_packet_at, and packets_per_second
    Given the sim status output
    When it is parsed
    Then each adapter entry SHALL include: connected, last_packet_at, and packets_per_second

  @AC-425.3
  Scenario: Status polling refreshes every 1 second when --watch flag is used
    Given `flightctl sim status --watch` is running
    When 3 seconds elapse
    Then the output SHALL have been refreshed at least 3 times

  @AC-425.4
  Scenario: Disconnected adapters show last disconnection time
    Given an adapter that is currently disconnected
    When `flightctl sim status` is executed
    Then the output SHALL include the timestamp of the last disconnection

  @AC-425.5
  Scenario: Status output is available in JSON format with --json flag
    Given `flightctl sim status --json` is executed
    When the output is parsed
    Then it SHALL be valid JSON containing the same fields as the human-readable output

  @AC-425.6
  Scenario: Exit code is 0 if any adapter is connected, 1 if none are connected
    Given the adapter connection state
    When `flightctl sim status` returns
    Then the exit code SHALL be 0 if at least one adapter is connected, or 1 if none are
