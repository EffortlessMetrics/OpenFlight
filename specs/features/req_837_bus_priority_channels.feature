Feature: Bus Priority Channels
  As a flight simulation enthusiast
  I want bus priority channels
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Priority channels processed before normal
    Given the system is configured for bus priority channels
    When the feature is exercised
    Then bus supports priority channels that are processed before normal channels

  Scenario: Preempt normal messages under backpressure
    Given the system is configured for bus priority channels
    When the feature is exercised
    Then priority messages preempt queued normal messages under backpressure

  Scenario: Assign priority at channel creation
    Given the system is configured for bus priority channels
    When the feature is exercised
    Then channel priority level is assigned at channel creation time

  Scenario: Bound priority count to prevent starvation
    Given the system is configured for bus priority channels
    When the feature is exercised
    Then priority channel count is bounded to prevent starvation of normal traffic
