Feature: Axis Jitter Suppression
  As a flight simulation enthusiast
  I want the axis engine to suppress sub-threshold jitter
  So that small unwanted movements in noisy hardware do not affect control output

  Background:
    Given the OpenFlight service is running

  Scenario: Jitter suppressor ignores changes smaller than configured threshold
    Given a jitter suppression threshold is configured for an axis
    When the axis input changes by less than the threshold
    Then the axis output remains unchanged

  Scenario: Threshold is configurable per axis in profile
    When a profile is authored
    Then each axis entry supports an optional jitter suppression threshold field

  Scenario: Suppression applies before deadzone stage
    Given both jitter suppression and a deadzone are configured for an axis
    When the axis pipeline executes
    Then jitter suppression is applied before the deadzone stage

  Scenario: Suppression does not accumulate error over time
    Given jitter suppression is active for an axis
    When many sub-threshold input changes are applied consecutively
    Then no cumulative positional error accumulates in the axis output
