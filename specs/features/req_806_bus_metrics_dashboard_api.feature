Feature: Bus Metrics Dashboard API
  As a flight simulation enthusiast
  I want bus metrics dashboard api
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose message throughput rate
    Given the system is configured for bus metrics dashboard api
    When the feature is exercised
    Then bus exposes message throughput rate via structured API endpoint

  Scenario: Expose subscriber count and health per channel
    Given the system is configured for bus metrics dashboard api
    When the feature is exercised
    Then bus exposes subscriber count and health status per channel

  Scenario: Return metrics in JSON with timestamps
    Given the system is configured for bus metrics dashboard api
    When the feature is exercised
    Then metrics API returns data in JSON format with timestamps

  Scenario: API response latency under 50ms
    Given the system is configured for bus metrics dashboard api
    When the feature is exercised
    Then aPI response latency does not exceed 50ms under normal load
