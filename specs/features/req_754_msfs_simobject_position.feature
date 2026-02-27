Feature: MSFS SimObject Position
  As a flight simulation enthusiast
  I want msfs simobject position
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Read latitude and longitude
    Given the system is configured for msfs simobject position
    When the feature is exercised
    Then simconnect adapter reads simobject latitude and longitude

  Scenario: Read altitude and heading
    Given the system is configured for msfs simobject position
    When the feature is exercised
    Then simconnect adapter reads simobject altitude and heading

  Scenario: Position update rate
    Given the system is configured for msfs simobject position
    When the feature is exercised
    Then position data is updated at least once per second

  Scenario: Publish position on bus
    Given the system is configured for msfs simobject position
    When the feature is exercised
    Then position data is published on the event bus
