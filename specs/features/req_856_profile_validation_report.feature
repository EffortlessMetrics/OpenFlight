Feature: Profile Validation Report
  As a flight simulation enthusiast
  I want profile validation report
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Validation produces a structured report with errors and warnings
    Given the system is configured for profile validation report
    When the feature is exercised
    Then validation produces a structured report with errors and warnings

  Scenario: Report includes actionable suggestions for each validation issue
    Given the system is configured for profile validation report
    When the feature is exercised
    Then report includes actionable suggestions for each validation issue

  Scenario: Validation checks axis ranges, curve continuity, and binding conflicts
    Given the system is configured for profile validation report
    When the feature is exercised
    Then validation checks axis ranges, curve continuity, and binding conflicts

  Scenario: Report is available in JSON and human-readable text formats
    Given the system is configured for profile validation report
    When the feature is exercised
    Then report is available in JSON and human-readable text formats
