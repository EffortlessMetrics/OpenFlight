Feature: CLI Benchmark Command
  As a flight simulation enthusiast
  I want cli benchmark command
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Benchmark subcommand
    Given the system is configured for cli benchmark command
    When the feature is exercised
    Then cli provides a benchmark subcommand for axis engine performance

  Scenario: Report latency percentiles
    Given the system is configured for cli benchmark command
    When the feature is exercised
    Then benchmark reports mean, p50, p99, and max tick latency

  Scenario: Configurable duration
    Given the system is configured for cli benchmark command
    When the feature is exercised
    Then benchmark duration is configurable

  Scenario: Export results to JSON
    Given the system is configured for cli benchmark command
    When the feature is exercised
    Then benchmark results are exportable to json
