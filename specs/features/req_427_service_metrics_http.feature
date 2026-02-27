@REQ-427 @product
Feature: Service Metrics HTTP Export — Expose Prometheus-Format /metrics Endpoint

  @AC-427.1
  Scenario: Service exposes /metrics endpoint when metrics_http feature is enabled
    Given the service is started with the metrics_http feature enabled
    When an HTTP GET request is sent to /metrics
    Then the response status SHALL be 200 OK

  @AC-427.2
  Scenario: Endpoint returns valid Prometheus text format with correct Content-Type
    Given the /metrics endpoint is reachable
    When the response is received
    Then the Content-Type header SHALL be "text/plain; version=0.0.4; charset=utf-8"
    And the body SHALL conform to the Prometheus text exposition format

  @AC-427.3
  Scenario: Axis processing rate counter is included in metrics output
    Given the service has processed at least one RT tick
    When the /metrics endpoint is queried
    Then the response SHALL contain a counter metric for axis processing rate

  @AC-427.4
  Scenario: Metrics server bind address and port are configurable
    Given the service config specifies a non-default metrics bind address and port
    When the service starts
    Then the /metrics endpoint SHALL be reachable on the configured address and port
