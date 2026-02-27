Feature: CLI Profile Validate Command
  As a flight simulation enthusiast
  I want cli profile validate command
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Validate subcommand exists
    Given the system is configured for cli profile validate command
    When the feature is exercised
    Then cli provides a validate subcommand for profile yaml files

  Scenario: Report schema errors with line numbers
    Given the system is configured for cli profile validate command
    When the feature is exercised
    Then validation reports schema errors with line numbers

  Scenario: Validation does not activate profile
    Given the system is configured for cli profile validate command
    When the feature is exercised
    Then validation does not apply or activate the profile

  Scenario: Non-zero exit on failure
    Given the system is configured for cli profile validate command
    When the feature is exercised
    Then exit code is non-zero when validation fails
