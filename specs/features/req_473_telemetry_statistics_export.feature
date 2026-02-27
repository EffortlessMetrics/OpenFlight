@REQ-473 @product
Feature: Telemetry Statistics Export — Periodic Statistics Logging  @AC-473.1
  Scenario: Statistics include axis update rate, error rate, and latency percentiles
    Given the telemetry statistics exporter is running
    When statistics are collected over one interval
    Then the exported data SHALL include axis update rate, error rate, and p50/p95/p99 latencies  @AC-473.2
  Scenario: Statistics are exported to a configurable log file
    Given the service config specifies a statistics log file path
    When the export interval elapses
    Then statistics SHALL be written to the configured file in a structured format  @AC-473.3
  Scenario: Statistics export interval is configurable
    Given a statistics export interval of 30 seconds is configured
    When the service runs
    Then statistics SHALL be exported approximately every 30 seconds  @AC-473.4
  Scenario: Statistics are accessible via flightctl stats command
    Given the service is running and has collected statistics
    When `flightctl stats` is executed
    Then the command SHALL display the latest statistics snapshot to stdout
