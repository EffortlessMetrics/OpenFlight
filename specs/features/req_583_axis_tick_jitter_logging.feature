Feature: Axis Engine Tick Jitter Logging
  As a flight simulation enthusiast
  I want the axis engine to log tick timing for jitter analysis
  So that I can diagnose real-time performance issues

  Background:
    Given the OpenFlight service is running
    And the axis engine is active at 250 Hz

  Scenario: Each tick duration is recorded in a rolling window
    When the axis engine completes a tick
    Then the tick duration is stored in a rolling window of recent tick durations

  Scenario: p50, p95, p99 jitter statistics are computed on demand
    Given the rolling tick window contains at least 100 samples
    When jitter statistics are requested
    Then the response includes p50, p95, and p99 tick duration values

  Scenario: Jitter statistics are available via gRPC RPC
    When a client calls the GetJitterStats gRPC RPC
    Then the response contains p50, p95, and p99 jitter values in microseconds

  Scenario: Jitter exceeding threshold triggers a warning log
    Given the jitter warning threshold is set to 500 microseconds
    When a tick duration exceeds 500 microseconds
    Then a warning log entry is emitted containing the tick duration and threshold
