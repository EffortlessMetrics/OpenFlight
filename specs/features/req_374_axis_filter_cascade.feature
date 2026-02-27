@REQ-374 @product
Feature: Axis Input Filtering Cascade — Chain Multiple Filters in Sequence

  @AC-374.1
  Scenario: Multiple filter stages can be chained
    Given an axis profile with EMA, jitter suppression, and window average stages configured
    When the axis processes an input value
    Then all filter stages SHALL be applied in sequence

  @AC-374.2
  Scenario: Filter order is specified in the profile configuration
    Given a profile specifying filter stages in a defined order
    When the cascade is built
    Then the filters SHALL be applied in the exact order specified in the profile

  @AC-374.3
  Scenario: Cascade produces the same result as manually chaining filter calls
    Given an axis with a cascade of filters A then B then C
    When an input is processed through the cascade
    Then the output SHALL equal the result of applying A, then B, then C individually

  @AC-374.4
  Scenario: Empty cascade is a pass-through
    Given an axis with no filter stages configured
    When an input value is processed
    Then the output SHALL equal the input unchanged

  @AC-374.5
  Scenario: Each filter stage contributes its latency to the cascade report
    Given a cascade with multiple filter stages each having a known latency
    When the cascade latency is reported
    Then the total reported latency SHALL be the sum of all stage latencies

  @AC-374.6
  Scenario: Property test — cascade output remains in [-1, 1] for valid inputs
    Given an axis with an arbitrary cascade of filters
    When inputs in [-1.0, 1.0] are processed via property testing
    Then every cascade output SHALL remain within [-1.0, 1.0]
