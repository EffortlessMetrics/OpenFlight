@REQ-524 @product
Feature: Multi-Adapter Fallback — Resilient Simulator Adapter Selection  @AC-524.1
  Scenario: Primary adapter failure triggers fallback to secondary adapter
    Given the service is configured with a primary MSFS adapter and a secondary X-Plane adapter
    When the primary MSFS adapter fails with a connection error
    Then the service SHALL activate the secondary X-Plane adapter within 5 seconds  @AC-524.2
  Scenario: Fallback order is configurable in service config
    Given a service config specifying adapter priority [dcs, xplane, simconnect]
    When the service starts and DCS is unavailable
    Then the service SHALL attempt X-Plane as the next adapter in priority order  @AC-524.3
  Scenario: Fallback event is published on flight-bus
    Given the service has fallen back from the primary adapter to a secondary adapter
    When the fallback transition completes
    Then a AdapterFallback event SHALL be published on the flight-bus with the reason  @AC-524.4
  Scenario: Service recovers primary adapter when it becomes available again
    Given the service is operating on a secondary adapter after a primary failure
    When the primary adapter becomes reachable again
    Then the service SHALL reconnect to the primary adapter and resume normal operation
