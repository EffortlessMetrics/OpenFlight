Feature: Navigation Data Feed
  As a flight simulation enthusiast
  I want navigation data feed
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Navigation data from simulator is available for instrument panel display
    Given the system is configured for navigation data feed
    When the feature is exercised
    Then navigation data from simulator is available for instrument panel display

  Scenario: Data feed includes heading, altitude, airspeed, and vertical speed
    Given the system is configured for navigation data feed
    When the feature is exercised
    Then data feed includes heading, altitude, airspeed, and vertical speed

  Scenario: Navigation data update rate is configurable per instrument requirement
    Given the system is configured for navigation data feed
    When the feature is exercised
    Then navigation data update rate is configurable per instrument requirement

  Scenario: Data feed handles simulator pause and time acceleration gracefully
    Given the system is configured for navigation data feed
    When the feature is exercised
    Then data feed handles simulator pause and time acceleration gracefully