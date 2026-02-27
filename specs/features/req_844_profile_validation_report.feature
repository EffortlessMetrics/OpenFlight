Feature: Profile Validation Report
  As a flight simulation enthusiast
  I want profile validation report
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate detailed compliance report
    Given the system is configured for profile validation report
    When the feature is exercised
    Then profile validator generates a detailed compliance report

  Scenario: List errors with field paths
    Given the system is configured for profile validation report
    When the feature is exercised
    Then report lists all validation errors with field paths and descriptions

  Scenario: Include deprecation warnings
    Given the system is configured for profile validation report
    When the feature is exercised
    Then report includes warnings for deprecated or suboptimal settings

  Scenario: Support JSON and readable formats
    Given the system is configured for profile validation report
    When the feature is exercised
    Then report output supports JSON and human-readable formats
