Feature: Bus Event History
  As a flight simulation enthusiast
  I want bus event history
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Maintain rolling buffer of recent events
    Given the system is configured for bus event history
    When the feature is exercised
    Then bus maintains a rolling buffer of recent events for debugging

  Scenario: Buffer size is configurable
    Given the system is configured for bus event history
    When the feature is exercised
    Then event history buffer size is configurable via service settings

  Scenario: Query history by type and time range
    Given the system is configured for bus event history
    When the feature is exercised
    Then historical events are queryable by event type and time range

  Scenario: FIFO eviction when capacity reached
    Given the system is configured for bus event history
    When the feature is exercised
    Then buffer eviction follows FIFO order when capacity is reached
