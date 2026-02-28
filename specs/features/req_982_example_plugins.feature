Feature: Example Plugins
  As a flight simulation enthusiast
  I want example plugins
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Reference implementations exist for WASM, native fast-path, and service tiers
    Given the system is configured for example plugins
    When the feature is exercised
    Then reference implementations exist for WASM, native fast-path, and service tiers

  Scenario: Each example plugin includes build instructions and test harness
    Given the system is configured for example plugins
    When the feature is exercised
    Then each example plugin includes build instructions and test harness

  Scenario: Example plugins demonstrate common patterns like axis modification and event handling
    Given the system is configured for example plugins
    When the feature is exercised
    Then example plugins demonstrate common patterns like axis modification and event handling

  Scenario: Examples are validated in CI to ensure they compile against current SDK
    Given the system is configured for example plugins
    When the feature is exercised
    Then examples are validated in CI to ensure they compile against current SDK