Feature: Custom Device Profiles
  As a flight simulation enthusiast
  I want custom device profiles
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Users can create custom device definitions for unsupported hardware
    Given the system is configured for custom device profiles
    When the feature is exercised
    Then users can create custom device definitions for unsupported hardware

  Scenario: Custom profiles define axis mappings and button assignments
    Given the system is configured for custom device profiles
    When the feature is exercised
    Then custom profiles define axis mappings and button assignments

  Scenario: Custom device profiles are validated against the device profile schema
    Given the system is configured for custom device profiles
    When the feature is exercised
    Then custom device profiles are validated against the device profile schema

  Scenario: Custom profiles can be shared and imported from community repositories
    Given the system is configured for custom device profiles
    When the feature is exercised
    Then custom profiles can be shared and imported from community repositories