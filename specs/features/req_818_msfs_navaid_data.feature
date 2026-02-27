Feature: MSFS NavAid Data
  As a flight simulation enthusiast
  I want msfs navaid data
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Expose VOR identifier and bearing
    Given the system is configured for msfs navaid data
    When the feature is exercised
    Then simConnect adapter exposes VOR station identifier and bearing

  Scenario: Read ILS localizer and glideslope deviation
    Given the system is configured for msfs navaid data
    When the feature is exercised
    Then iLS localizer and glideslope deviation are readable as variables

  Scenario: Provide NDB relative bearing from ADF
    Given the system is configured for msfs navaid data
    When the feature is exercised
    Then nDB relative bearing is available when ADF receiver is tuned

  Scenario: Refresh rate matches polling interval
    Given the system is configured for msfs navaid data
    When the feature is exercised
    Then navAid data refresh rate matches the SimConnect polling interval
