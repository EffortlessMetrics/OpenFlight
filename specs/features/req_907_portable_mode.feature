Feature: Portable Mode
  As a flight simulation enthusiast
  I want portable mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Application runs without installation when portable marker file exists
    Given the system is configured for portable mode
    When the feature is exercised
    Then application runs without installation when portable marker file exists

  Scenario: Portable mode stores all config alongside the executable directory
    Given the system is configured for portable mode
    When the feature is exercised
    Then portable mode stores all config alongside the executable directory

  Scenario: Portable mode works from USB drive without leaving traces on host
    Given the system is configured for portable mode
    When the feature is exercised
    Then portable mode works from USB drive without leaving traces on host

  Scenario: Portable mode disables auto-start and update features by default
    Given the system is configured for portable mode
    When the feature is exercised
    Then portable mode disables auto-start and update features by default
