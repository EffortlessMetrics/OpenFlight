Feature: FFB Turbulence Scaling
  As a flight simulation enthusiast
  I want ffb turbulence scaling
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Scale turbulence with indicated airspeed
    Given the system is configured for ffb turbulence scaling
    When the feature is exercised
    Then fFB turbulence intensity scales proportionally with indicated airspeed

  Scenario: Configurable scaling curve in profile
    Given the system is configured for ffb turbulence scaling
    When the feature is exercised
    Then scaling curve is configurable via profile parameters

  Scenario: Blend smoothly with other FFB effects
    Given the system is configured for ffb turbulence scaling
    When the feature is exercised
    Then turbulence effect blends smoothly with other active FFB effects

  Scenario: Respect safety envelope force limits
    Given the system is configured for ffb turbulence scaling
    When the feature is exercised
    Then scaling respects FFB safety envelope maximum force limits
