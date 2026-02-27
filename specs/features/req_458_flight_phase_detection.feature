@REQ-458 @product
Feature: Flight Phase Detection — Detect and Broadcast Current Phase of Flight

  @AC-458.1
  Scenario: All defined flight phases are recognised
    Given a simulator providing telemetry covering the full flight envelope
    When telemetry values matching each phase threshold are received in sequence
    Then the service SHALL detect and report each of: parked, taxiing, takeoff, climb, cruise, descent, approach, landing

  @AC-458.2
  Scenario: Phase is determined from simulator telemetry variables
    Given a connected simulator publishing airspeed, altitude rate, and gear state
    When airspeed exceeds the takeoff threshold and altitude rate becomes positive
    Then the detected phase SHALL transition to climb

  @AC-458.3
  Scenario: Phase transition triggers profile phase activation
    Given a profile with phase-specific axis curves for approach
    When the flight phase transitions to approach
    Then the approach axis curve configuration SHALL be atomically activated in the RT spine

  @AC-458.4
  Scenario: Phase detection hysteresis prevents rapid oscillation
    Given the current phase is cruise and airspeed is oscillating around the descent threshold
    When airspeed crosses the threshold multiple times within the hysteresis window
    Then the phase SHALL NOT oscillate and SHALL only change after sustained threshold breach
