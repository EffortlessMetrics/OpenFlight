Feature: Axis Chain Benchmark Regression
  As a developer
  I want automated benchmark regression tests for the axis pipeline
  So that performance regressions are caught before release

  Background:
    Given the Criterion benchmark suite is configured for flight-axis

  Scenario: Full AxisChain process() loop is benchmarked
    When the Criterion benchmark "axis_chain_full_process" is executed
    Then it measures the end-to-end latency of AxisChain::process()
    And the result is recorded in the benchmark baseline

  Scenario: p95 per-axis latency meets threshold
    When the benchmark suite runs for axis pipeline throughput
    Then the p95 latency per axis is at or below 10 microseconds
    And a regression report is generated

  Scenario: Benchmark failure blocks release merge
    Given the benchmark baseline records a p95 of 8 microseconds
    When the current run produces a p95 of 15 microseconds
    Then the benchmark is marked as a regression
    And the CI pipeline returns a non-zero exit code
