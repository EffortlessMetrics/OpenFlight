Feature: Metrics Export
  As a flight simulation enthusiast
  I want metrics export
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Prometheus-compatible metrics endpoint exposes system metrics
    Given the system is configured for metrics export
    When the feature is exercised
    Then prometheus-compatible metrics endpoint exposes system metrics

  Scenario: Metrics include axis processing latency, jitter, and throughput counters
    Given the system is configured for metrics export
    When the feature is exercised
    Then metrics include axis processing latency, jitter, and throughput counters

  Scenario: Device connection status and error counts are exposed as metrics
    Given the system is configured for metrics export
    When the feature is exercised
    Then device connection status and error counts are exposed as metrics

  Scenario: Metrics endpoint supports HTTP scraping on configurable port
    Given the system is configured for metrics export
    When the feature is exercised
    Then metrics endpoint supports HTTP scraping on configurable port
