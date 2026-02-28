Feature: Performance Benchmark Suite
  As a flight simulation enthusiast
  I want performance benchmark suite
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Standardized benchmarks measure axis processing throughput
    Given the system is configured for performance benchmark suite
    When the feature is exercised
    Then standardized benchmarks measure axis processing throughput

  Scenario: Benchmark results are compared against checked-in baseline values
    Given the system is configured for performance benchmark suite
    When the feature is exercised
    Then benchmark results are compared against checked-in baseline values

  Scenario: Regressions beyond a configurable threshold fail the benchmark run
    Given the system is configured for performance benchmark suite
    When the feature is exercised
    Then regressions beyond a configurable threshold fail the benchmark run

  Scenario: Benchmark suite runs in isolation to minimize measurement noise
    Given the system is configured for performance benchmark suite
    When the feature is exercised
    Then benchmark suite runs in isolation to minimize measurement noise
