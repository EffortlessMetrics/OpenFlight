Feature: Performance Counters
  As a flight simulation enthusiast
  I want performance counters
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Windows performance counters are registered for perfmon integration
    Given the system is configured for performance counters
    When the feature is exercised
    Then windows performance counters are registered for perfmon integration

  Scenario: Linux perf integration exposes custom counters for system monitoring
    Given the system is configured for performance counters
    When the feature is exercised
    Then linux perf integration exposes custom counters for system monitoring

  Scenario: Counters track tick rate, processing time, and queue depths
    Given the system is configured for performance counters
    When the feature is exercised
    Then counters track tick rate, processing time, and queue depths

  Scenario: Performance counter registration is automatic during service startup
    Given the system is configured for performance counters
    When the feature is exercised
    Then performance counter registration is automatic during service startup
