Feature: Axis Engine Safe Shutdown
  As a flight simulation enthusiast
  I want the axis engine to complete current tick before stopping
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Current tick completed before stop
    Given the axis engine is processing a tick
    When a shutdown signal is received
    Then the current tick completes before stopping

  Scenario: Pending outputs flushed
    Given a shutdown is in progress
    When the final tick completes
    Then pending output values are flushed to consumers

  Scenario: Shutdown within 100ms
    Given a shutdown signal is received
    When the engine shuts down
    Then shutdown completes within 100ms

  Scenario: Shutdown reason logged
    Given the axis engine shuts down
    When the shutdown completes
    Then the reason is logged for diagnostics
