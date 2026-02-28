Feature: Load Testing Framework
  As a flight simulation enthusiast
  I want load testing framework
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Stress tests exercise the service under sustained high device counts
    Given the system is configured for load testing framework
    When the feature is exercised
    Then stress tests exercise the service under sustained high device counts

  Scenario: Load profiles are configurable for duration, concurrency, and rate
    Given the system is configured for load testing framework
    When the feature is exercised
    Then load profiles are configurable for duration, concurrency, and rate

  Scenario: Framework reports latency percentiles and error rates under load
    Given the system is configured for load testing framework
    When the feature is exercised
    Then framework reports latency percentiles and error rates under load

  Scenario: Load test results are compared against defined SLA thresholds
    Given the system is configured for load testing framework
    When the feature is exercised
    Then load test results are compared against defined SLA thresholds
