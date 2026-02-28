Feature: Input Validation
  As a flight simulation enthusiast
  I want input validation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: All external inputs are validated against schema before processing
    Given the system is configured for input validation
    When the feature is exercised
    Then all external inputs are validated against schema before processing

  Scenario: Profile file parsing rejects malformed or oversized input with error
    Given the system is configured for input validation
    When the feature is exercised
    Then profile file parsing rejects malformed or oversized input with error

  Scenario: IPC message fields are bounds-checked before deserialization
    Given the system is configured for input validation
    When the feature is exercised
    Then iPC message fields are bounds-checked before deserialization

  Scenario: Device descriptor parsing sanitizes strings to prevent injection
    Given the system is configured for input validation
    When the feature is exercised
    Then device descriptor parsing sanitizes strings to prevent injection
