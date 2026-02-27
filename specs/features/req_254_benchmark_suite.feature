@REQ-254 @infra
Feature: Benchmark suite measures axis pipeline performance at 250Hz  @AC-254.1
  Scenario: Criterion benchmark measures single-axis pipeline stages
    Given the flight-axis benchmark is compiled
    When the single-axis benchmark runs the calibration to EMA to rate-limiter to trim pipeline
    Then Criterion SHALL report throughput and latency statistics for the full stage chain  @AC-254.2
  Scenario: Benchmark runs 1000 iterations for stable p99 estimate
    Given the Criterion benchmark configuration for the axis pipeline
    When the benchmark executes
    Then Criterion SHALL perform at least 1000 iterations before reporting the p99 latency estimate  @AC-254.3
  Scenario: Benchmark result compared to baseline stored in CI artifacts
    Given a prior CI run has stored a benchmark baseline artifact
    When the current CI benchmark run completes
    Then the new results SHALL be compared to the stored baseline and a delta report SHALL be produced  @AC-254.4
  Scenario: Regression threshold p99 latency under 2 microseconds per axis
    Given the axis pipeline benchmark has completed
    When the p99 latency result is evaluated against the regression threshold
    Then the CI step SHALL fail if the p99 latency exceeds 2 microseconds per axis  @AC-254.5
  Scenario: Benchmark flamegraph generated and published as CI artifact
    Given the CI benchmark job is configured with flamegraph profiling enabled
    When the benchmark completes
    Then a flamegraph SVG SHALL be generated and published as a named CI artifact  @AC-254.6
  Scenario: Benchmark suite documented in benchmarking how-to guide
    Given the docs/how-to/benchmarking.md file in the repository
    When the file is inspected
    Then it SHALL contain instructions for running the benchmark suite locally and interpreting results
