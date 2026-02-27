Feature: CLI Scripting Mode
  As a flight simulation enthusiast
  I want cli scripting mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Non-interactive scripting mode suppresses prompts and progress bars
    Given the system is configured for cli scripting mode
    When the feature is exercised
    Then non-interactive scripting mode suppresses prompts and progress bars

  Scenario: Exit codes follow standard conventions for success and error types
    Given the system is configured for cli scripting mode
    When the feature is exercised
    Then exit codes follow standard conventions for success and error types

  Scenario: Scripting mode output is stable and machine-parseable
    Given the system is configured for cli scripting mode
    When the feature is exercised
    Then scripting mode output is stable and machine-parseable

  Scenario: Environment variable selects scripting mode without a CLI flag
    Given the system is configured for cli scripting mode
    When the feature is exercised
    Then environment variable selects scripting mode without a CLI flag
