Feature: CLI Profile Wizard
  As a flight simulation enthusiast
  I want cli profile wizard
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Interactive wizard guides creation of a new profile from scratch
    Given the system is configured for cli profile wizard
    When the feature is exercised
    Then interactive wizard guides creation of a new profile from scratch

  Scenario: Wizard suggests axis mappings based on detected devices
    Given the system is configured for cli profile wizard
    When the feature is exercised
    Then wizard suggests axis mappings based on detected devices

  Scenario: Created profile is validated before being saved to disk
    Given the system is configured for cli profile wizard
    When the feature is exercised
    Then created profile is validated before being saved to disk

  Scenario: Wizard supports pre-built templates for common aircraft types
    Given the system is configured for cli profile wizard
    When the feature is exercised
    Then wizard supports pre-built templates for common aircraft types
