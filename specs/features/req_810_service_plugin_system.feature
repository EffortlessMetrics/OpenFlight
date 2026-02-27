Feature: Service Plugin System
  As a flight simulation enthusiast
  I want service plugin system
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Discover and load plugins from directory
    Given the system is configured for service plugin system
    When the feature is exercised
    Then service discovers and loads plugin modules from a configured directory

  Scenario: Plugins declare capabilities via manifest
    Given the system is configured for service plugin system
    When the feature is exercised
    Then plugins declare required capabilities via a manifest file

  Scenario: Log plugin lifecycle events
    Given the system is configured for service plugin system
    When the feature is exercised
    Then plugin lifecycle events (load/unload/error) are logged

  Scenario: Isolate faulty plugins from main service
    Given the system is configured for service plugin system
    When the feature is exercised
    Then faulty plugins are isolated and cannot crash the main service
