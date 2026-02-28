Feature: Delta Updates
  As a flight simulation enthusiast
  I want delta updates
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Update system generates binary diffs between consecutive versions
    Given the system is configured for delta updates
    When the feature is exercised
    Then update system generates binary diffs between consecutive versions

  Scenario: Delta patches reduce download size by at least 50 percent versus full packages
    Given the system is configured for delta updates
    When the feature is exercised
    Then delta patches reduce download size by at least 50 percent versus full packages

  Scenario: Delta application verifies source version hash before patching
    Given the system is configured for delta updates
    When the feature is exercised
    Then delta application verifies source version hash before patching

  Scenario: Full package fallback occurs when delta patch fails verification
    Given the system is configured for delta updates
    When the feature is exercised
    Then full package fallback occurs when delta patch fails verification
