Feature: Axis Engine Burst Mode
  As a flight simulation enthusiast
  I want the axis engine to support burst processing for catch-up
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Burst processing catches up delayed ticks
    Given the axis engine has fallen behind schedule
    When burst mode is activated
    Then delayed ticks are processed in a burst

  Scenario: Configurable maximum burst size
    Given burst mode is active
    When the catch-up exceeds the configured maximum
    Then processing stops at the configured burst limit

  Scenario: Deterministic output order maintained
    Given burst processing is active
    When multiple ticks are processed
    Then output order is deterministic

  Scenario: Burst events are logged
    Given burst mode is activated
    When the burst completes
    Then the burst event is logged for diagnostics
