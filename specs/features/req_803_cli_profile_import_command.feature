Feature: CLI Profile Import Command
  As a flight simulation enthusiast
  I want cli profile import command
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Import profile from local file path
    Given the system is configured for cli profile import command
    When the feature is exercised
    Then cLI accepts a local file path to import a profile

  Scenario: Import profile from URL
    Given the system is configured for cli profile import command
    When the feature is exercised
    Then cLI accepts a URL to download and import a profile

  Scenario: Validate imported profile against schema
    Given the system is configured for cli profile import command
    When the feature is exercised
    Then imported profiles are validated against the current schema before saving

  Scenario: Prompt on conflicts with existing profiles
    Given the system is configured for cli profile import command
    When the feature is exercised
    Then import conflicts with existing profiles prompt for overwrite confirmation
