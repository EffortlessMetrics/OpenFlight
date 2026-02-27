@REQ-194 @infra
Feature: Performance benchmarks guard against RT spine regressions  @AC-194.1
  Scenario: Criterion benchmark covers axis pipeline at 250Hz
    Given the axis pipeline benchmark suite is present
    When the benchmarks are run
    Then a Criterion benchmark measuring the 250Hz axis processing loop SHALL execute and produce results  @AC-194.2
  Scenario: Benchmark latency percentiles tracked against baseline
    Given a baseline benchmark result is stored for the axis pipeline
    When a new benchmark run completes
    Then p50, p95, and p99 latency values SHALL be compared against the stored baseline  @AC-194.3
  Scenario: Greater than 20% p99 regression fails nightly CI
    Given the nightly CI benchmark job is running
    When the measured p99 latency exceeds the baseline by more than 20 percent
    Then the CI job SHALL fail and report the regression  @AC-194.4
  Scenario: Benchmark results stored as CI artifact
    Given a CI benchmark run has completed
    When the job finishes
    Then the benchmark result data SHALL be stored as a downloadable artifact in CI  @AC-194.5
  Scenario: Flamegraph generated during benchmark run
    Given the benchmark suite supports profiling output
    When a benchmark run is executed with flamegraph generation enabled
    Then a flamegraph SHALL be produced and saved alongside the benchmark results  @AC-194.6
  Scenario: Per-tick allocation count measured and bounded
    Given the axis pipeline benchmark is running with allocation tracking enabled
    When one tick of the RT processing loop is measured
    Then the benchmark SHALL report zero heap allocations per tick on the RT hot path
