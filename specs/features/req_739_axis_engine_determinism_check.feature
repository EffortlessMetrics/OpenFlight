Feature: Axis Engine Determinism Check
  As a flight simulation enthusiast
  I want the axis engine to verify deterministic output for same input
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Deterministic output verified
    Given the axis engine is in diagnostic mode
    When identical inputs are processed twice
    Then the outputs are identical

  Scenario: Non-determinism logged
    Given a non-deterministic output is detected
    When the check completes
    Then a warning is logged

  Scenario: Periodic check in diagnostic mode
    Given diagnostic mode is enabled
    When the configured check interval elapses
    Then a determinism check runs automatically

  Scenario: Zero allocation on RT path
    Given a determinism check is running
    When the check executes on the RT path
    Then no additional heap allocation occurs
