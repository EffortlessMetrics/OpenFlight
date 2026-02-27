@REQ-412 @product
Feature: Service Metrics HTTP Endpoint — Expose Metrics in Prometheus Format

  @AC-412.1
  Scenario: GET /metrics returns Prometheus-formatted text
    Given the service is running with the metrics endpoint enabled
    When GET /metrics is requested
    Then the response SHALL be Prometheus-formatted text containing all counter and gauge metrics

  @AC-412.2
  Scenario: Metrics include required fields
    Given the metrics endpoint is responding
    When the response is parsed
    Then it SHALL include: axis_count, sim_connected, device_count, and tick_rate_hz

  @AC-412.3
  Scenario: HTTP server binds to 127.0.0.1:9090 by default and is configurable
    Given a service started with default configuration
    When the metrics endpoint address is checked
    Then it SHALL bind to 127.0.0.1:9090 and the address SHALL be configurable

  @AC-412.4
  Scenario: /metrics endpoint responds within 50 ms
    Given the metrics endpoint under normal load
    When a GET /metrics request is issued
    Then the response SHALL arrive within 50 ms

  @AC-412.5
  Scenario: Metrics are updated atomically with no partial reads
    Given the metrics HTTP endpoint
    When a client reads the metrics
    Then the values SHALL be atomically consistent (no partial state visible)

  @AC-412.6
  Scenario: Integration test — start service, poll /metrics, verify axis_count > 0
    Given a running service with at least one active axis
    When GET /metrics is polled
    Then axis_count SHALL be greater than 0
