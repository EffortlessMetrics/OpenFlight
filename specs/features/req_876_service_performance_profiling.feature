Feature: Service Performance Profiling
  As a flight simulation enthusiast
  I want service performance profiling
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Built-in performance sampling captures CPU and latency metrics
    Given the system is configured for service performance profiling
    When the feature is exercised
    Then built-in performance sampling captures CPU and latency metrics

  Scenario: Profiling can be started and stopped via CLI command
    Given the system is configured for service performance profiling
    When the feature is exercised
    Then profiling can be started and stopped via CLI command

  Scenario: Profile data is exported in a standard format for analysis tools
    Given the system is configured for service performance profiling
    When the feature is exercised
    Then profile data is exported in a standard format for analysis tools

  Scenario: Profiling overhead does not exceed 2 percent of baseline CPU usage
    Given the system is configured for service performance profiling
    When the feature is exercised
    Then profiling overhead does not exceed 2 percent of baseline CPU usage
