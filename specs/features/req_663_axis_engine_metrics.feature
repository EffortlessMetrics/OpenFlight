Feature: Axis Engine Metrics Endpoint
  As a flight simulation enthusiast
  I want the axis engine to expose metrics via a Prometheus endpoint
  So that I can monitor real-time axis performance with standard tooling

  Background:
    Given the OpenFlight service is running with the metrics endpoint enabled

  Scenario: Axis tick rate is exposed as a gauge metric
    When the Prometheus metrics endpoint is scraped
    Then the axis engine tick rate is present as a gauge metric

  Scenario: Per-axis clamp count is exposed as counter
    Given one or more axes have produced clamped output values
    When the Prometheus metrics endpoint is scraped
    Then a per-axis clamp counter metric is present for each affected axis

  Scenario: Jitter p99 is exposed as gauge
    When the Prometheus metrics endpoint is scraped
    Then the axis engine jitter p99 value is present as a gauge metric

  Scenario: Metrics endpoint path is /metrics on configured port
    Given a metrics port is configured
    When an HTTP GET request is made to /metrics on that port
    Then the Prometheus-formatted metrics response is returned
