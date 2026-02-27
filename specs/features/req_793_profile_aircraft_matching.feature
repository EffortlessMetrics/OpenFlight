Feature: Profile Aircraft Matching
  As a flight simulation enthusiast
  I want profile aircraft matching
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Glob pattern matching
    Given the system is configured for profile aircraft matching
    When the feature is exercised
    Then profiles match aircraft by glob pattern not just exact name

  Scenario: Wildcards and character classes
    Given the system is configured for profile aircraft matching
    When the feature is exercised
    Then pattern matching supports wildcards and character classes

  Scenario: Most specific pattern wins
    Given the system is configured for profile aircraft matching
    When the feature is exercised
    Then most specific matching pattern takes precedence

  Scenario: Log match results
    Given the system is configured for profile aircraft matching
    When the feature is exercised
    Then match results are logged for troubleshooting
