Feature: CLI Device Test Mode
  As a flight simulation enthusiast
  I want cli device test mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Run diagnostic sequence on specified device
    Given the system is configured for cli device test mode
    When the feature is exercised
    Then cLI runs a diagnostic test sequence on a specified device

  Scenario: Exercise all axes, buttons, and LEDs
    Given the system is configured for cli device test mode
    When the feature is exercised
    Then test sequence exercises all axes, buttons, and LEDs on the device

  Scenario: Display structured pass/fail report
    Given the system is configured for cli device test mode
    When the feature is exercised
    Then test results are displayed in a structured pass/fail report

  Scenario: Timeout after configurable duration
    Given the system is configured for cli device test mode
    When the feature is exercised
    Then device test mode times out after a configurable duration
