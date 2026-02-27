Feature: Autopilot Integration
  As a flight simulation enthusiast
  I want autopilot integration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Autopilot state is read from simulator for panel LED indication
    Given the system is configured for autopilot integration
    When the feature is exercised
    Then autopilot state is read from simulator for panel LED indication

  Scenario: AP mode changes update panel indicators within 100ms of sim state change
    Given the system is configured for autopilot integration
    When the feature is exercised
    Then aP mode changes update panel indicators within 100ms of sim state change

  Scenario: Panel buttons can engage and disengage autopilot modes in the simulator
    Given the system is configured for autopilot integration
    When the feature is exercised
    Then panel buttons can engage and disengage autopilot modes in the simulator

  Scenario: Autopilot altitude, heading, and speed settings are displayed on panels
    Given the system is configured for autopilot integration
    When the feature is exercised
    Then autopilot altitude, heading, and speed settings are displayed on panels