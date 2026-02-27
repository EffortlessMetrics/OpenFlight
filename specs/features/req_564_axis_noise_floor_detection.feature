Feature: Axis Noise Floor Detection
  As a flight simulation enthusiast
  I want the axis engine to detect and suppress hardware noise floor
  So that small electrical noise does not cause unwanted axis movement

  Background:
    Given the OpenFlight service is running
    And a joystick axis "THROTTLE" is connected and calibrated

  Scenario: Noise floor detector measures signal variance when control is at rest
    Given the "THROTTLE" axis has not moved for 2 seconds
    When the noise floor detector samples the axis signal
    Then the measured variance is recorded as the noise floor for "THROTTLE"

  Scenario: Automatic micro-deadzone is applied above detected noise floor
    Given the detected noise floor for "THROTTLE" is 0.003
    When the axis pipeline is configured for automatic noise suppression
    Then a micro-deadzone of at least 0.003 is applied centred on the resting value

  Scenario: Noise floor measurement is stored per device in calibration store
    When the noise floor for "THROTTLE" is measured
    Then the measurement is persisted in the calibration store keyed by device serial and axis ID

  Scenario: Noise floor can be measured via CLI calibration command
    When the operator runs "flightctl calibrate noise-floor --device THROTTLE_QUADRANT --axis THROTTLE"
    Then the service measures the noise floor and reports the result in the CLI output
