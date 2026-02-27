Feature: Profile Import Validation
  As a flight simulation enthusiast
  I want profile import validation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Schema validate before activation
    Given the system is configured for profile import validation
    When the feature is exercised
    Then imported profiles are schema-validated before activation

  Scenario: Reject unknown fields
    Given the system is configured for profile import validation
    When the feature is exercised
    Then validation rejects profiles with unknown fields

  Scenario: Error includes field path and type
    Given the system is configured for profile import validation
    When the feature is exercised
    Then validation errors include field path and expected type

  Scenario: Import valid profiles unmodified
    Given the system is configured for profile import validation
    When the feature is exercised
    Then valid profiles are imported without modification
