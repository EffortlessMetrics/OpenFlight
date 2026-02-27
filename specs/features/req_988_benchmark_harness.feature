Feature: Benchmark Harness
  As a flight simulation enthusiast
  I want benchmark harness
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Standardized performance benchmarks cover axis processing and FFB synthesis
    Given the system is configured for benchmark harness
    When the feature is exercised
    Then standardized performance benchmarks cover axis processing and FFB synthesis

  Scenario: Benchmark results are comparable across runs with statistical analysis
    Given the system is configured for benchmark harness
    When the feature is exercised
    Then benchmark results are comparable across runs with statistical analysis

  Scenario: Regression detection alerts when performance degrades beyond threshold
    Given the system is configured for benchmark harness
    When the feature is exercised
    Then regression detection alerts when performance degrades beyond threshold

  Scenario: Benchmark harness supports custom scenarios defined in configuration files
    Given the system is configured for benchmark harness
    When the feature is exercised
    Then benchmark harness supports custom scenarios defined in configuration files