@REQ-188 @infra
Feature: Service exposes health endpoint for monitoring and watchdog integration  @AC-188.1
  Scenario: GET /health returns 200 when service is healthy
    Given the OpenFlight service is running and its RT spine is ticking normally
    When an HTTP GET request is made to /health
    Then the response status code SHALL be 200  @AC-188.2
  Scenario: Health response includes RT spine metrics
    Given the OpenFlight service is running and returning a 200 health response
    When the /health response body is parsed
    Then the body SHALL include the current RT spine tick rate and p99 jitter metric  @AC-188.3
  Scenario: Health response includes per-adapter status
    Given the OpenFlight service is running with one or more simulator adapters configured
    When the /health response body is parsed
    Then the body SHALL include the connection status of each adapter as connected, disconnected, or error  @AC-188.4
  Scenario: /health/ready returns 503 before first RT tick
    Given the OpenFlight service has started but the RT spine has not yet completed its first tick
    When an HTTP GET request is made to /health/ready
    Then the response status code SHALL be 503  @AC-188.5
  Scenario: Watchdog restarts service after consecutive health failures
    Given the watchdog is configured to query /health every 5 seconds
    When /health returns a non-200 response for 3 consecutive queries
    Then the watchdog SHALL initiate a service restart  @AC-188.6
  Scenario: Health metrics available at Prometheus-compatible endpoint
    Given the OpenFlight service is running
    When an HTTP GET request is made to /metrics
    Then the response SHALL contain health metrics in Prometheus exposition format
