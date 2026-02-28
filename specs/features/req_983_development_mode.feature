Feature: Development Mode
  As a flight simulation enthusiast
  I want development mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Verbose logging mode provides detailed trace output for all system operations
    Given the system is configured for development mode
    When the feature is exercised
    Then verbose logging mode provides detailed trace output for all system operations

  Scenario: Hot-reload enables profile and plugin changes without service restart
    Given the system is configured for development mode
    When the feature is exercised
    Then hot-reload enables profile and plugin changes without service restart

  Scenario: Development mode disables RT priority to allow debugging without elevated privileges
    Given the system is configured for development mode
    When the feature is exercised
    Then development mode disables RT priority to allow debugging without elevated privileges

  Scenario: Mock simulator connections are available for testing without running sims
    Given the system is configured for development mode
    When the feature is exercised
    Then mock simulator connections are available for testing without running sims