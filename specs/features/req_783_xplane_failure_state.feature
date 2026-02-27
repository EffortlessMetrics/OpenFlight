Feature: X-Plane Failure State
  As a flight simulation enthusiast
  I want x-plane failure state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Track failure states via datarefs
    Given the system is configured for x-plane failure state
    When the feature is exercised
    Then x-plane adapter tracks system failure states via datarefs

  Scenario: Engine electrical hydraulic failures
    Given the system is configured for x-plane failure state
    When the feature is exercised
    Then failure states include engine, electrical, and hydraulic systems

  Scenario: Publish failure changes on bus
    Given the system is configured for x-plane failure state
    When the feature is exercised
    Then failure state changes are published on the event bus

  Scenario: Include in status report
    Given the system is configured for x-plane failure state
    When the feature is exercised
    Then active failures are included in the status report
