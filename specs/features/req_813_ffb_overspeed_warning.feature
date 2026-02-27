Feature: FFB Overspeed Warning
  As a flight simulation enthusiast
  I want ffb overspeed warning
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate vibration when exceeding Vne
    Given the system is configured for ffb overspeed warning
    When the feature is exercised
    Then fFB generates a distinct vibration pattern when airspeed exceeds Vne

  Scenario: Increase intensity with excess speed
    Given the system is configured for ffb overspeed warning
    When the feature is exercised
    Then vibration intensity increases as airspeed further exceeds the limit

  Scenario: Cease vibration when below Vne
    Given the system is configured for ffb overspeed warning
    When the feature is exercised
    Then overspeed vibration ceases immediately when airspeed returns below Vne

  Scenario: Configurable threshold per aircraft profile
    Given the system is configured for ffb overspeed warning
    When the feature is exercised
    Then overspeed threshold is configurable per aircraft profile
