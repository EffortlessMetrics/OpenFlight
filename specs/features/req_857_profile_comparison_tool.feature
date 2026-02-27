Feature: Profile Comparison Tool
  As a flight simulation enthusiast
  I want profile comparison tool
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Two profiles can be compared side-by-side showing all differences
    Given the system is configured for profile comparison tool
    When the feature is exercised
    Then two profiles can be compared side-by-side showing all differences

  Scenario: Comparison highlights added, removed, and modified bindings
    Given the system is configured for profile comparison tool
    When the feature is exercised
    Then comparison highlights added, removed, and modified bindings

  Scenario: Axis curve differences are reported with numerical deltas
    Given the system is configured for profile comparison tool
    When the feature is exercised
    Then axis curve differences are reported with numerical deltas

  Scenario: Comparison output is available via CLI and service API
    Given the system is configured for profile comparison tool
    When the feature is exercised
    Then comparison output is available via CLI and service API
