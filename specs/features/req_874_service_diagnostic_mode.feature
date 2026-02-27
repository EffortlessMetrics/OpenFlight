Feature: Service Diagnostic Mode
  As a flight simulation enthusiast
  I want service diagnostic mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Diagnostic mode enables enhanced logging for all subsystems
    Given the system is configured for service diagnostic mode
    When the feature is exercised
    Then diagnostic mode enables enhanced logging for all subsystems

  Scenario: Mode can be activated and deactivated at runtime via CLI
    Given the system is configured for service diagnostic mode
    When the feature is exercised
    Then mode can be activated and deactivated at runtime via CLI

  Scenario: Diagnostic output includes timing traces for RT spine ticks
    Given the system is configured for service diagnostic mode
    When the feature is exercised
    Then diagnostic output includes timing traces for RT spine ticks

  Scenario: Diagnostic mode automatically disables after a configurable timeout
    Given the system is configured for service diagnostic mode
    When the feature is exercised
    Then diagnostic mode automatically disables after a configurable timeout
