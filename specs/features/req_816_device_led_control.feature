Feature: Device LED Control
  As a flight simulation enthusiast
  I want device led control
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Set LED state based on sim conditions
    Given the system is configured for device led control
    When the feature is exercised
    Then service sets device LED state based on mapped simulator conditions

  Scenario: Define LED mappings in device profile
    Given the system is configured for device led control
    When the feature is exercised
    Then lED mappings are defined in the device profile configuration

  Scenario: Apply changes within one cycle
    Given the system is configured for device led control
    When the feature is exercised
    Then lED state changes are applied within one processing cycle

  Scenario: Ignore unsupported commands with debug log
    Given the system is configured for device led control
    When the feature is exercised
    Then unsupported LED commands are silently ignored with a debug log
