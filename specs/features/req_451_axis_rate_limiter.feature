@REQ-451 @product
Feature: Axis Rate Limiter — Smooth Rapid Axis Changes via Per-Axis Rate Limiting

  @AC-451.1
  Scenario: Rate limiter constrains maximum change per tick
    Given an axis configured with a rate limit of 2.0 units per second
    When the physical input jumps from 0.0 to 1.0 in a single tick at 250Hz
    Then the output SHALL change by no more than 0.008 per tick

  @AC-451.2
  Scenario: Rate limit is configurable per axis
    Given two axes with rate limits of 1.0 and 10.0 units per second respectively
    When both axes receive an identical step input
    Then the axis with the lower rate limit SHALL reach full deflection more slowly

  @AC-451.3
  Scenario: Rate limiting is applied after curves and before output
    Given an axis with a response curve and a rate limit configured
    When a step input is applied
    Then the output value SHALL reflect the curve transformation before rate limiting is enforced

  @AC-451.4
  Scenario: Rate limiter state resets when axis is released to center
    Given an axis currently rate-limited mid-transition toward 1.0
    When the physical input returns to 0.0
    Then the rate limiter state SHALL reset so the next deflection starts from zero without carry-over
