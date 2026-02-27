Feature: Joystick Axis Calibration Visualization
  As a flight simulation enthusiast
  I want a live visualization when calibrating joystick axes
  So that I can accurately calibrate my hardware

  Background:
    Given the OpenFlight service is running
    And a joystick is connected

  Scenario: flightctl axis calibrate shows live input visualization
    When the user runs "flightctl axis calibrate"
    Then a live visualization of the axis input is displayed

  Scenario: Visualization shows min, max, center, and current position
    When the calibration visualization is active
    Then it shows the current minimum, maximum, center, and live position values

  Scenario: Calibration prompts user through physical movement sequence
    When the calibration starts
    Then the CLI prompts the user to move the axis to its minimum, maximum, and center positions in sequence

  Scenario: Completed calibration is saved to calibration store
    Given the user has completed the calibration sequence
    When the user confirms the calibration
    Then the calibration data is saved to the calibration store
