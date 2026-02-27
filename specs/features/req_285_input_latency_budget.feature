@REQ-285 @product
Feature: Input latency budget enforcement with per-stage measurement, Prometheus metric, and CI benchmark  @AC-285.1
  Scenario: Input latency from HID read to axis output is under 5ms p99
    Given the service is processing HID input at nominal load
    When latency samples are collected across 10000 ticks
    Then the p99 latency from HID read to axis output SHALL be less than 5 milliseconds  @AC-285.2
  Scenario: Pipeline stage contribution is measured per-tick
    Given the axis pipeline is running
    When a single tick completes
    Then the elapsed time for each stage SHALL be recorded and available in the per-tick diagnostics  @AC-285.3
  Scenario: Latency exceeding budget generates a warning log
    Given the configured latency budget for a device class is 5ms
    When a tick's end-to-end latency exceeds that budget
    Then a structured warning log entry SHALL be emitted containing the measured latency and budget value  @AC-285.4
  Scenario: p99 latency metric is exposed via Prometheus
    Given the service is running with metrics enabled
    When the Prometheus scrape endpoint is queried
    Then it SHALL include a histogram metric for input latency with p99 quantile available  @AC-285.5
  Scenario: Latency budget is configurable per device class
    Given a profile specifying different latency budgets for joystick and throttle device classes
    When the service loads that profile
    Then each device class SHALL enforce its own configured latency budget independently  @AC-285.6
  Scenario: CI benchmark test validates sub-5ms latency in simulation
    Given the CI benchmark harness is run against the simulated axis pipeline
    When the benchmark completes
    Then the measured p99 latency SHALL be less than 5ms and the benchmark SHALL exit with success
