@REQ-318 @product
Feature: Streaming Metrics Export  @AC-318.1
  Scenario: Service exposes metrics on /metrics in Prometheus format
    Given the metrics endpoint is enabled in the service configuration
    When an HTTP GET request is made to /metrics
    Then the service SHALL respond with metrics data in the Prometheus text exposition format  @AC-318.2
  Scenario: Metrics include axis values device count pipeline latency and error rates
    Given the metrics endpoint is active and the service is processing input
    When the metrics payload is inspected
    Then the metrics SHALL include current axis values, connected device count, pipeline latency, and error rates  @AC-318.3
  Scenario: Metrics endpoint is configurable for port path and enable/disable
    Given the service configuration file
    When the metrics endpoint settings are adjusted
    Then the service SHALL honour configurable port, path, and enabled/disabled settings for the metrics endpoint  @AC-318.4
  Scenario: Authentication for metrics endpoint is optional via bearer token
    Given the metrics endpoint has an optional bearer token configured
    When a request arrives without a valid token
    Then the service SHALL reject unauthenticated requests when a token is configured but allow access when no token is set  @AC-318.5
  Scenario: Metrics are updated at most every 100ms (rate-limited writes)
    Given the service is processing input at 250Hz
    When the metrics store is updated
    Then individual metric values SHALL be written to the export store at most once every 100 milliseconds  @AC-318.6
  Scenario: CLI can display live metrics via flightctl metrics
    Given the service is running with metrics enabled
    When the user runs flightctl metrics
    Then the CLI SHALL connect to the metrics endpoint and display live metric values in the terminal
