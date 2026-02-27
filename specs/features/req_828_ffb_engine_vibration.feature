Feature: FFB Engine Vibration
  As a flight simulation enthusiast
  I want ffb engine vibration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Amplitude proportional to RPM
    Given the system is configured for ffb engine vibration
    When the feature is exercised
    Then fFB simulates engine vibration with amplitude proportional to RPM

  Scenario: Frequency matches configurable RPM harmonic
    Given the system is configured for ffb engine vibration
    When the feature is exercised
    Then vibration frequency matches a configurable harmonic of engine RPM

  Scenario: Sum vibrations from multiple engines
    Given the system is configured for ffb engine vibration
    When the feature is exercised
    Then multi-engine aircraft sum vibrations from all running engines

  Scenario: Smooth ramp to zero on shutdown
    Given the system is configured for ffb engine vibration
    When the feature is exercised
    Then engine shutdown smoothly ramps vibration to zero
