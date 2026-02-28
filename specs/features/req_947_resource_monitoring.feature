Feature: Resource Monitoring
  As a flight simulation enthusiast
  I want resource monitoring
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Service tracks its own CPU usage and reports via metrics endpoint
    Given the system is configured for resource monitoring
    When the feature is exercised
    Then service tracks its own CPU usage and reports via metrics endpoint

  Scenario: Memory usage is monitored with alerts on approaching configured limits
    Given the system is configured for resource monitoring
    When the feature is exercised
    Then memory usage is monitored with alerts on approaching configured limits

  Scenario: Disk usage for logs and data is tracked and reported
    Given the system is configured for resource monitoring
    When the feature is exercised
    Then disk usage for logs and data is tracked and reported

  Scenario: Resource metrics are sampled at configurable intervals
    Given the system is configured for resource monitoring
    When the feature is exercised
    Then resource metrics are sampled at configurable intervals
