@REQ-710
Feature: Axis Relative Mode
  @AC-710.1
  Scenario: Axis can operate in relative incremental mode instead of absolute
    Given the system is configured for REQ-710
    When the feature condition is met
    Then axis can operate in relative incremental mode instead of absolute

  @AC-710.2
  Scenario: Relative mode accumulates delta values from input changes
    Given the system is configured for REQ-710
    When the feature condition is met
    Then relative mode accumulates delta values from input changes

  @AC-710.3
  Scenario: Accumulated value is clamped to configured output range
    Given the system is configured for REQ-710
    When the feature condition is met
    Then accumulated value is clamped to configured output range

  @AC-710.4
  Scenario: Relative mode sensitivity multiplier is configurable
    Given the system is configured for REQ-710
    When the feature condition is met
    Then relative mode sensitivity multiplier is configurable
