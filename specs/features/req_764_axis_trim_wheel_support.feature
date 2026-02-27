Feature: Axis Trim Wheel Support
  As a flight simulation enthusiast
  I want axis trim wheel support
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Process incremental trim input
    Given the system is configured for axis trim wheel support
    When the feature is exercised
    Then axis engine processes trim wheel incremental input correctly

  Scenario: Accumulate from deltas
    Given the system is configured for axis trim wheel support
    When the feature is exercised
    Then trim position accumulates from incremental deltas

  Scenario: Bounded trim range
    Given the system is configured for axis trim wheel support
    When the feature is exercised
    Then trim range is bounded by configurable limits

  Scenario: Reset on profile change
    Given the system is configured for axis trim wheel support
    When the feature is exercised
    Then trim position resets on profile change or explicit reset
