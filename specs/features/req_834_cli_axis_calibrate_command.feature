Feature: CLI Axis Calibrate Command
  As a flight simulation enthusiast
  I want cli axis calibrate command
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Interactive calibration wizard
    Given the system is configured for cli axis calibrate command
    When the feature is exercised
    Then cLI guides users through an interactive axis calibration wizard

  Scenario: Capture min, center, and max per axis
    Given the system is configured for cli axis calibrate command
    When the feature is exercised
    Then calibration captures minimum, center, and maximum for each axis

  Scenario: Auto-save results to device profile
    Given the system is configured for cli axis calibrate command
    When the feature is exercised
    Then calibration results are saved to the device profile automatically

  Scenario: Validate range meets resolution threshold
    Given the system is configured for cli axis calibrate command
    When the feature is exercised
    Then wizard validates that captured range meets minimum resolution threshold
