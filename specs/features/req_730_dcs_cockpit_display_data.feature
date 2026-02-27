Feature: DCS Cockpit Display Data
  As a flight simulation enthusiast
  I want the DCS adapter to support cockpit display indicators
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Display indicators exposed
    Given DCS is running with cockpit display data
    When the DCS adapter receives export data
    Then cockpit display indicators are exposed

  Scenario: Indicators mapped to LEDs
    Given indicator data is available
    When LED mappings are configured in the profile
    Then indicators drive panel LED outputs

  Scenario: Updates at DCS export rate
    Given DCS is exporting display data
    When the export cycle completes
    Then display data updates at the DCS export rate

  Scenario: Missing indicators default to off
    Given an expected indicator is not in the export data
    When the adapter processes the data
    Then the missing indicator defaults to off state
