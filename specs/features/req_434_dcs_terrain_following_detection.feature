@REQ-434 @product
Feature: DCS World Terrain-Following Detection — Detect TFR Activation from Export Telemetry

  @AC-434.1
  Scenario: Adapter reads TFR armed status from DCS export telemetry
    Given the DCS Export adapter is receiving telemetry
    When the export data contains a TFR armed flag
    Then the adapter SHALL parse and expose the TFR armed status

  @AC-434.2
  Scenario: TFR activation triggers profile phase transition to low-level
    Given a profile with a low-level phase defined
    When TFR armed status transitions from false to true
    Then the service SHALL initiate a phase transition to the low-level phase

  @AC-434.3
  Scenario: Phase transition event is published on flight-bus within 20ms
    Given TFR activation is detected
    When the event is published
    Then the phase_transition event SHALL appear on the flight-bus within 20ms of detection

  @AC-434.4
  Scenario: TFR status is included in adapter health metrics
    Given the DCS Export adapter health metrics are queried
    When the metrics are inspected
    Then a tfr_armed gauge metric SHALL reflect the current TFR armed state
