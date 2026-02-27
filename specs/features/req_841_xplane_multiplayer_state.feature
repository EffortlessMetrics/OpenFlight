Feature: X-Plane Multiplayer State
  As a flight simulation enthusiast
  I want x-plane multiplayer state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Detect active multiplayer session
    Given the system is configured for x-plane multiplayer state
    When the feature is exercised
    Then x-Plane adapter detects whether a multiplayer session is active

  Scenario: Expose connected aircraft count
    Given the system is configured for x-plane multiplayer state
    When the feature is exercised
    Then number of connected multiplayer aircraft is exposed as a variable

  Scenario: Publish multiplayer state changes
    Given the system is configured for x-plane multiplayer state
    When the feature is exercised
    Then multiplayer state changes are published to the event bus

  Scenario: Handle disconnect gracefully
    Given the system is configured for x-plane multiplayer state
    When the feature is exercised
    Then adapter handles multiplayer disconnect gracefully without errors
