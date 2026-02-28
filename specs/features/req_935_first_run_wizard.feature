Feature: First-Run Wizard
  As a flight simulation enthusiast
  I want first-run wizard
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: First launch presents guided setup wizard for initial configuration
    Given the system is configured for first-run wizard
    When the feature is exercised
    Then first launch presents guided setup wizard for initial configuration

  Scenario: Wizard auto-detects connected devices and installed simulators
    Given the system is configured for first-run wizard
    When the feature is exercised
    Then wizard auto-detects connected devices and installed simulators

  Scenario: Wizard creates initial profile based on detected hardware
    Given the system is configured for first-run wizard
    When the feature is exercised
    Then wizard creates initial profile based on detected hardware

  Scenario: Wizard can be skipped and re-launched from settings at any time
    Given the system is configured for first-run wizard
    When the feature is exercised
    Then wizard can be skipped and re-launched from settings at any time
