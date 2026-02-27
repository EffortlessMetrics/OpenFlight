@REQ-305 @product
Feature: War Thunder Telemetry Binding

  @AC-305.1
  Scenario: Service reads War Thunder state via HTTP telemetry endpoint
    Given War Thunder is running with its localhost telemetry server active
    When the service is started with a War Thunder profile
    Then the service SHALL fetch vehicle state by issuing HTTP GET requests to the telemetry endpoint

  @AC-305.2
  Scenario: Telemetry endpoint is polled at 20Hz
    Given the service is connected to the War Thunder telemetry server at localhost:8111/state.json
    When the service is running normally
    Then the service SHALL issue approximately 20 HTTP GET requests per second to the state endpoint

  @AC-305.3
  Scenario: Aircraft type triggers profile switch
    Given the service is polling War Thunder telemetry
    When the telemetry response indicates a change in aircraft type
    Then the service SHALL switch to the profile associated with that aircraft type

  @AC-305.4
  Scenario: Connected and disconnected state is tracked
    Given the service is polling the War Thunder telemetry endpoint
    When the endpoint stops responding
    Then the service SHALL record a disconnected state and emit a status event

  @AC-305.5
  Scenario: Ground vehicle telemetry is supported with a separate profile
    Given a profile configured for a War Thunder ground vehicle
    When the telemetry response indicates a ground vehicle is active
    Then the service SHALL activate the ground vehicle profile and process its telemetry fields

  @AC-305.6
  Scenario: Telemetry failure triggers fallback to idle profile
    Given the service has been receiving War Thunder telemetry
    When the telemetry endpoint returns an error or becomes unreachable
    Then the service SHALL activate the idle fallback profile until telemetry is restored
