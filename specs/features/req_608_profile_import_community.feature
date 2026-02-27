Feature: Profile Import from Community Sources
  As a flight simulation enthusiast
  I want to import profiles from community repositories
  So that I can benefit from community-created configurations

  Background:
    Given the OpenFlight service is running
    And flightctl is installed

  Scenario: flightctl profile import URL downloads and validates profile
    When the user runs "flightctl profile import <URL>"
    Then the profile is downloaded from the URL
    And the profile is validated against the schema

  Scenario: Import verifies schema version compatibility
    Given a community profile with schema version 2
    And the service supports schema version 2
    When the profile is imported
    Then the import succeeds with a compatibility confirmation

  Scenario: Imported profiles are stored in user profile directory
    When the user imports a community profile
    Then the profile is stored in the user profile directory

  Scenario: Import requires explicit user confirmation
    When the user runs "flightctl profile import <URL>"
    Then the CLI prompts the user to confirm the import before proceeding
    And the profile is only saved after the user confirms
