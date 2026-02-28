Feature: CLI Performance Report
  As a flight simulation enthusiast
  I want cli performance report
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Performance analysis report is generated with a single CLI command
    Given the system is configured for cli performance report
    When the feature is exercised
    Then performance analysis report is generated with a single CLI command

  Scenario: Report includes RT spine latency, jitter, and throughput metrics
    Given the system is configured for cli performance report
    When the feature is exercised
    Then report includes RT spine latency, jitter, and throughput metrics

  Scenario: Report compares current metrics against historical baselines
    Given the system is configured for cli performance report
    When the feature is exercised
    Then report compares current metrics against historical baselines

  Scenario: Output supports both summary and detailed breakdown views
    Given the system is configured for cli performance report
    When the feature is exercised
    Then output supports both summary and detailed breakdown views
