Feature: Error Messages
  As a flight simulation enthusiast
  I want error messages
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Error messages describe the problem in non-technical language
    Given the system is configured for error messages
    When the feature is exercised
    Then error messages describe the problem in non-technical language

  Scenario: Each error message includes a suggested action for resolution
    Given the system is configured for error messages
    When the feature is exercised
    Then each error message includes a suggested action for resolution

  Scenario: Error codes are included for support reference and searchability
    Given the system is configured for error messages
    When the feature is exercised
    Then error codes are included for support reference and searchability

  Scenario: Critical errors are visually distinct from warnings and info messages
    Given the system is configured for error messages
    When the feature is exercised
    Then critical errors are visually distinct from warnings and info messages
