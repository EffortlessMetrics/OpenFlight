Feature: Device Hot-Swap Support
  As a flight simulation enthusiast
  I want device hot-swap support
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Detect disconnect and mark offline
    Given the system is configured for device hot-swap support
    When the feature is exercised
    Then service detects device disconnection and marks it offline without crashing

  Scenario: Detect reconnect and reinitialize
    Given the system is configured for device hot-swap support
    When the feature is exercised
    Then service detects device reconnection and re-initializes it automatically

  Scenario: Resume axis mappings on reconnect
    Given the system is configured for device hot-swap support
    When the feature is exercised
    Then active axis mappings for reconnected devices resume without user intervention

  Scenario: Log hot-swap events with identity and timestamp
    Given the system is configured for device hot-swap support
    When the feature is exercised
    Then hot-swap events are logged with device identity and timestamp
