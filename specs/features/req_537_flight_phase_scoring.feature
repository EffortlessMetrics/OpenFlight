Feature: Flight Phase Telemetry Scoring
  As a flight simulation enthusiast
  I want the auto-switch service to score phase transitions using telemetry
  So that profile switching is accurate and stable

  Background:
    Given the OpenFlight service is running
    And the auto-switch service is active with a scoring-based phase detector

  Scenario: Phase transition requires confidence threshold
    Given the confidence threshold is configured to 0.85
    When airspeed, altitude, and gear state telemetry indicate a landing approach
    But the confidence score is only 0.72
    Then no phase transition fires

  Scenario: Phase transition fires when confidence threshold is met
    Given the confidence threshold is configured to 0.80
    When telemetry consistently scores above 0.80 for the "approach" phase
    Then the phase transitions to "approach"
    And the transition is recorded in the phase score history

  Scenario: Phase score history available in diagnostics
    Given several phase transitions have occurred during the session
    When I query diagnostics via "flightctl diagnostics phase-scores"
    Then the output includes a history of phase scores and transition timestamps
