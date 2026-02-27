Feature: Profile Phase-of-Flight Rules
  As a flight simulation enthusiast
  I want profile phase-of-flight rules
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Auto-switch on phase of flight
    Given the system is configured for profile phase-of-flight rules
    When the feature is exercised
    Then profiles auto-switch based on detected phase of flight (taxi, takeoff, cruise, approach, landing)

  Scenario: Detect phase from altitude, speed, gear, flaps
    Given the system is configured for profile phase-of-flight rules
    When the feature is exercised
    Then phase detection uses altitude, airspeed, and gear/flap state

  Scenario: Hysteresis on phase transitions
    Given the system is configured for profile phase-of-flight rules
    When the feature is exercised
    Then phase transitions include a configurable hysteresis to avoid rapid switching

  Scenario: Manual override disables auto-detection
    Given the system is configured for profile phase-of-flight rules
    When the feature is exercised
    Then manual phase override disables automatic detection until re-enabled
