Feature: Profile Template System
  As a flight simulation enthusiast
  I want profile template system
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Define base templates for inheritance
    Given the system is configured for profile template system
    When the feature is exercised
    Then profile system supports defining base templates that other profiles inherit from

  Scenario: Child profiles override explicit fields only
    Given the system is configured for profile template system
    When the feature is exercised
    Then child profiles override only the fields they explicitly specify

  Scenario: Limit inheritance depth to prevent cycles
    Given the system is configured for profile template system
    When the feature is exercised
    Then template inheritance depth is limited to prevent circular references

  Scenario: Resolve templates at load time
    Given the system is configured for profile template system
    When the feature is exercised
    Then template resolution is performed at profile load time, not runtime
