Feature: FFB Spring Effect Calibration
  As a flight simulation enthusiast
  I want the FFB engine to support spring effect calibration per aircraft
  So that force feedback feel matches the aircraft type accurately

  Background:
    Given the OpenFlight service is running with an FFB device connected

  Scenario: Spring effect gain is configurable per aircraft type in profile
    Given a profile with a spring effect gain defined for aircraft type "A320"
    When the FFB engine loads the profile for the A320 aircraft
    Then the spring effect gain matches the configured value for that aircraft type

  Scenario: Maximum spring force is bounded by safety envelope
    Given the FFB safety envelope defines a maximum spring force limit
    When a spring calibration value exceeding the limit is applied
    Then the FFB engine clamps the spring force to the safety envelope maximum

  Scenario: Spring calibration wizard is available in CLI
    When the command "flightctl ffb calibrate-spring" is run
    Then an interactive spring calibration wizard is started

  Scenario: Calibrated values are stored in calibration store
    Given a spring calibration wizard session completes successfully
    When the calibration is saved
    Then the calibrated spring values are persisted in the calibration store
