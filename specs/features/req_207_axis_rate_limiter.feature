@REQ-207 @product
Feature: Axis rate limiter prevents instantaneous large axis jumps  @AC-207.1
  Scenario: Rate limit specified as maximum change per tick
    Given an axis rate limiter configured with a limit of 0.05
    When the axis processes ticks at 250Hz
    Then the maximum change per tick SHALL be 0.05 (5% per tick)  @AC-207.2
  Scenario: Rate limiter applied after deadzone before output
    Given an axis pipeline with deadzone and rate limiter configured
    When a raw axis value passes through the pipeline
    Then the rate limiter SHALL be applied after deadzone processing and before output  @AC-207.3
  Scenario: Removing rate limit passes input unchanged
    Given an axis rate limiter configured with limit 0.0
    When any input value is processed
    Then the output SHALL equal the input without any rate limiting applied  @AC-207.4
  Scenario: Rate limiter state is per-axis and does not bleed between axes
    Given two axes each with independent rate limiter state
    When one axis undergoes rapid change
    Then the other axis output SHALL not be affected by the first axis rate limiter state  @AC-207.5
  Scenario: Rate limit configuration hot-reloaded from profile without service restart
    Given the service is running with a rate limit of 0.05 on an axis
    When the profile is updated to change the rate limit to 0.10
    Then the new rate limit SHALL take effect without restarting the service  @AC-207.6
  Scenario: Axis rate limit visualized in telemetry as slew rate
    Given an axis with rate limiting active
    When telemetry data is sampled
    Then the telemetry SHALL include a slew rate field reflecting the current rate limit
