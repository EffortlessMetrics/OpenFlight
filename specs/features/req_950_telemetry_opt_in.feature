Feature: Telemetry Opt-In
  As a flight simulation enthusiast
  I want telemetry opt-in
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Anonymous usage telemetry is disabled by default requiring explicit opt-in
    Given the system is configured for telemetry opt-in
    When the feature is exercised
    Then anonymous usage telemetry is disabled by default requiring explicit opt-in

  Scenario: Opt-in preference is configurable via CLI and UI settings
    Given the system is configured for telemetry opt-in
    When the feature is exercised
    Then opt-in preference is configurable via CLI and UI settings

  Scenario: Telemetry data is anonymized with no device or user identifiers
    Given the system is configured for telemetry opt-in
    When the feature is exercised
    Then telemetry data is anonymized with no device or user identifiers

  Scenario: User can view exactly what telemetry data would be sent before opting in
    Given the system is configured for telemetry opt-in
    When the feature is exercised
    Then user can view exactly what telemetry data would be sent before opting in
