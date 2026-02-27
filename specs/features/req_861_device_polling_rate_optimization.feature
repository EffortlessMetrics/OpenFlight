Feature: Device Polling Rate Optimization
  As a flight simulation enthusiast
  I want device polling rate optimization
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Polling interval is auto-tuned based on device response characteristics
    Given the system is configured for device polling rate optimization
    When the feature is exercised
    Then polling interval is auto-tuned based on device response characteristics

  Scenario: Optimization adjusts independently for each connected device
    Given the system is configured for device polling rate optimization
    When the feature is exercised
    Then optimization adjusts independently for each connected device

  Scenario: Tuning respects a minimum polling floor to prevent excessive CPU usage
    Given the system is configured for device polling rate optimization
    When the feature is exercised
    Then tuning respects a minimum polling floor to prevent excessive CPU usage

  Scenario: Polling rate changes are logged and visible in diagnostics
    Given the system is configured for device polling rate optimization
    When the feature is exercised
    Then polling rate changes are logged and visible in diagnostics
