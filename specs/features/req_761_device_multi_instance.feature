Feature: Device Multi-Instance Support
  As a flight simulation enthusiast
  I want device multi-instance support
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Distinguish by serial or path
    Given the system is configured for device multi-instance support
    When the feature is exercised
    Then service distinguishes multiple identical devices by serial number or path

  Scenario: Independent axis mappings per instance
    Given the system is configured for device multi-instance support
    When the feature is exercised
    Then each instance can have independent axis mappings

  Scenario: Persistent instance assignment
    Given the system is configured for device multi-instance support
    When the feature is exercised
    Then device instance assignment persists across reconnects

  Scenario: Profile references by instance ID
    Given the system is configured for device multi-instance support
    When the feature is exercised
    Then profile can reference devices by instance identifier
