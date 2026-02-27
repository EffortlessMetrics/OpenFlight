Feature: Device Auto-Calibration
  As a flight simulation enthusiast
  I want device auto-calibration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Auto-calibrate on first connection
    Given the system is configured for device auto-calibration
    When the feature is exercised
    Then service auto-calibrates devices by sampling range on first connection

  Scenario: Store and reuse calibration
    Given the system is configured for device auto-calibration
    When the feature is exercised
    Then calibration data is stored and reused on subsequent connections

  Scenario: Manual override via CLI
    Given the system is configured for device auto-calibration
    When the feature is exercised
    Then manual calibration override is available via cli

  Scenario: Report calibration status
    Given the system is configured for device auto-calibration
    When the feature is exercised
    Then calibration status is reported in device info
