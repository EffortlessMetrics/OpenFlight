Feature: Axis Engine Tick Budget Monitor
  As a flight simulation enthusiast
  I want axis engine tick budget monitor
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Monitor tick time budget
    Given the system is configured for axis engine tick budget monitor
    When the feature is exercised
    Then service monitors time spent in each axis engine tick

  Scenario: Log budget overrun warnings
    Given the system is configured for axis engine tick budget monitor
    When the feature is exercised
    Then budget overruns are logged as warnings

  Scenario: Configurable budget threshold
    Given the system is configured for axis engine tick budget monitor
    When the feature is exercised
    Then tick budget threshold is configurable

  Scenario: Expose metrics via telemetry
    Given the system is configured for axis engine tick budget monitor
    When the feature is exercised
    Then tick budget metrics are exposed via telemetry
