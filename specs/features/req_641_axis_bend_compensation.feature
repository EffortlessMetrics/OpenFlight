Feature: Axis Bend Compensation
  As a flight simulation enthusiast
  I want the axis engine to compensate for mechanical flex and bend
  So that my physical stick deflection accurately matches the intended axis output

  Background:
    Given the OpenFlight service is running

  Scenario: Bend compensation applies a correction curve to axis output
    Given a bend compensation curve is configured for an axis
    When an axis value is processed
    Then the correction curve is applied to the output

  Scenario: Correction curve is calibrated during setup wizard
    Given the setup wizard is launched
    When the bend calibration step is completed
    Then a correction curve is saved to the axis profile

  Scenario: Compensation magnitude is configurable per axis
    When different compensation magnitudes are set on two axes
    Then each axis applies its own independent compensation

  Scenario: Compensation is applied before other pipeline stages
    Given bend compensation and a response curve are both configured
    When an axis value is processed
    Then bend compensation is applied before the response curve stage
