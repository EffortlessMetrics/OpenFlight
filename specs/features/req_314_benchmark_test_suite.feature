@REQ-314 @product
Feature: Benchmark Test Suite  @AC-314.1
  Scenario: CI includes criterion benchmarks for axis pipeline throughput
    Given the CI pipeline configuration
    When the benchmark stage runs
    Then the CI SHALL execute criterion benchmarks measuring axis pipeline throughput  @AC-314.2
  Scenario: Pipeline benchmark measures single-axis chain at 250Hz
    Given the axis pipeline benchmark is configured
    When the benchmark runs a single-axis processing chain
    Then the benchmark SHALL measure throughput at the target 250Hz tick rate  @AC-314.3
  Scenario: Benchmark results are stored as CI artifacts
    Given a CI benchmark run completes
    When the pipeline finishes
    Then the benchmark output files SHALL be uploaded and stored as CI artifacts for comparison  @AC-314.4
  Scenario: Regression threshold pipeline must run in less than 100 microseconds per axis
    Given a criterion benchmark for a single axis pipeline pass
    When the benchmark result is evaluated
    Then the pipeline SHALL complete each axis processing pass in under 100 microseconds  @AC-314.5
  Scenario: Benchmark coverage includes deadzone curve EMA and all combined
    Given the benchmark suite is configured
    When benchmarks are enumerated
    Then the suite SHALL include individual benchmarks for deadzone, curve, EMA, and a combined all-stages scenario  @AC-314.6
  Scenario: Benchmark runs on Linux without Windows-specific overhead
    Given the CI benchmark runner configuration
    When the benchmark is scheduled
    Then the benchmark SHALL run on a Linux runner to avoid Windows-specific scheduling overhead
