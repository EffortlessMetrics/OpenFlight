Feature: Bus Event Filtering
  As a flight simulation enthusiast
  I want bus event filtering
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Register event type interest
    Given the system is configured for bus event filtering
    When the feature is exercised
    Then bus subscribers can register interest in specific event types

  Scenario: Only receive matching events
    Given the system is configured for bus event filtering
    When the feature is exercised
    Then filtered subscribers only receive matching events

  Scenario: Update filters without resubscribe
    Given the system is configured for bus event filtering
    When the feature is exercised
    Then filters can be updated without resubscribing

  Scenario: No allocation in filter evaluation
    Given the system is configured for bus event filtering
    When the feature is exercised
    Then filter evaluation does not allocate on the rt path
