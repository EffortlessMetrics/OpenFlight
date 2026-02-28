Feature: Profile Linter
  As a flight simulation enthusiast
  I want profile linter
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Static analysis detects common configuration errors in profile files
    Given the system is configured for profile linter
    When the feature is exercised
    Then static analysis detects common configuration errors in profile files

  Scenario: Linter checks include unused axis mappings, invalid references, and type mismatches
    Given the system is configured for profile linter
    When the feature is exercised
    Then linter checks include unused axis mappings, invalid references, and type mismatches

  Scenario: Lint warnings include suggested fixes with actionable guidance
    Given the system is configured for profile linter
    When the feature is exercised
    Then lint warnings include suggested fixes with actionable guidance

  Scenario: Profile linter integrates with CI pipeline for automated validation
    Given the system is configured for profile linter
    When the feature is exercised
    Then profile linter integrates with CI pipeline for automated validation