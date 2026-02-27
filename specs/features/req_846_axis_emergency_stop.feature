Feature: Axis Emergency Stop
  As a flight simulation enthusiast
  I want axis emergency stop
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Zero all outputs immediately on command
    Given the system is configured for axis emergency stop
    When the feature is exercised
    Then axis engine supports an emergency stop command that zeros all outputs immediately

  Scenario: Trigger via key binding or bus command
    Given the system is configured for axis emergency stop
    When the feature is exercised
    Then emergency stop is triggered via a configurable key binding or bus command

  Scenario: Require explicit reset to recover
    Given the system is configured for axis emergency stop
    When the feature is exercised
    Then recovery from emergency stop requires an explicit reset command

  Scenario: Log events with timestamp and source
    Given the system is configured for axis emergency stop
    When the feature is exercised
    Then emergency stop events are logged with timestamp and trigger source
