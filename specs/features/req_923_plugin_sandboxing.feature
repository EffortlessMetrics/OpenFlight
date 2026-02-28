Feature: Plugin Sandboxing
  As a flight simulation enthusiast
  I want plugin sandboxing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: WASM plugins execute in capability-limited sandbox with declared permissions
    Given the system is configured for plugin sandboxing
    When the feature is exercised
    Then wASM plugins execute in capability-limited sandbox with declared permissions

  Scenario: Sandbox prevents file system access beyond plugin-specific data directory
    Given the system is configured for plugin sandboxing
    When the feature is exercised
    Then sandbox prevents file system access beyond plugin-specific data directory

  Scenario: Plugin memory usage is bounded by configurable per-plugin limits
    Given the system is configured for plugin sandboxing
    When the feature is exercised
    Then plugin memory usage is bounded by configurable per-plugin limits

  Scenario: Sandbox violations are logged and terminate the offending plugin
    Given the system is configured for plugin sandboxing
    When the feature is exercised
    Then sandbox violations are logged and terminate the offending plugin
