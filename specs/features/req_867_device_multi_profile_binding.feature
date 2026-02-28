Feature: Device Multi-Profile Binding
  As a flight simulation enthusiast
  I want device multi-profile binding
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Different profiles can be assigned to individual connected devices
    Given the system is configured for device multi-profile binding
    When the feature is exercised
    Then different profiles can be assigned to individual connected devices

  Scenario: Per-device profile assignment persists across service restarts
    Given the system is configured for device multi-profile binding
    When the feature is exercised
    Then per-device profile assignment persists across service restarts

  Scenario: Removing a device falls back to the global active profile
    Given the system is configured for device multi-profile binding
    When the feature is exercised
    Then removing a device falls back to the global active profile

  Scenario: Profile-device bindings are displayed in the device status view
    Given the system is configured for device multi-profile binding
    When the feature is exercised
    Then profile-device bindings are displayed in the device status view
