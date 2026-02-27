Feature: Profile Export Format
  As a flight simulation enthusiast
  I want profile export format
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Export to portable JSON format
    Given the system is configured for profile export format
    When the feature is exercised
    Then profiles export to a portable JSON interchange format

  Scenario: Include schema version and metadata
    Given the system is configured for profile export format
    When the feature is exercised
    Then exported format includes schema version and metadata

  Scenario: Omit runtime state and computed fields
    Given the system is configured for profile export format
    When the feature is exercised
    Then export omits internal runtime state and computed fields

  Scenario: Re-import on compatible versions
    Given the system is configured for profile export format
    When the feature is exercised
    Then exported profiles can be re-imported on any compatible OpenFlight version
