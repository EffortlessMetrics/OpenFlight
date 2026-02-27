Feature: Axis Signal Quality Score
  As a flight simulation enthusiast
  I want each axis to have a computed signal quality score
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Score computed from 0 to 100
    Given an axis is active and receiving input
    When the signal quality is computed
    Then a score from 0 to 100 is produced

  Scenario: Score reflects noise and jitter
    Given an axis has varying signal quality
    When the quality score is computed
    Then it reflects noise level, jitter, and dead zone coverage

  Scenario: Score queryable via IPC
    Given the axis engine is running
    When a client queries signal quality via IPC
    Then the current quality score is returned in real time

  Scenario: Low score triggers warning
    Given an axis signal quality score drops below threshold
    When the score is evaluated
    Then a warning is surfaced in the CLI
