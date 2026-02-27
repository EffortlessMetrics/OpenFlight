Feature: Device Axis Resolution Detection
  As a flight simulation enthusiast
  I want device axis resolution detection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Detect bit resolution of each axis
    Given the system is configured for device axis resolution detection
    When the feature is exercised
    Then service detects the actual bit resolution of each device axis

  Scenario: Optimize scaling with detected resolution
    Given the system is configured for device axis resolution detection
    When the feature is exercised
    Then detected resolution is used to optimize input scaling calculations

  Scenario: Complete detection during initialization
    Given the system is configured for device axis resolution detection
    When the feature is exercised
    Then resolution detection completes during device initialization

  Scenario: Report resolution in device info
    Given the system is configured for device axis resolution detection
    When the feature is exercised
    Then detected resolution is reported in device info output
