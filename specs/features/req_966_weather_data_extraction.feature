Feature: Weather Data Extraction
  As a flight simulation enthusiast
  I want weather data extraction
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Wind speed and direction are extracted from simulator for FFB wind effects
    Given the system is configured for weather data extraction
    When the feature is exercised
    Then wind speed and direction are extracted from simulator for FFB wind effects

  Scenario: Turbulence intensity data drives force feedback vibration parameters
    Given the system is configured for weather data extraction
    When the feature is exercised
    Then turbulence intensity data drives force feedback vibration parameters

  Scenario: Weather data updates at minimum 4Hz for responsive force feedback
    Given the system is configured for weather data extraction
    When the feature is exercised
    Then weather data updates at minimum 4Hz for responsive force feedback

  Scenario: Missing weather data gracefully falls back to neutral force feedback state
    Given the system is configured for weather data extraction
    When the feature is exercised
    Then missing weather data gracefully falls back to neutral force feedback state