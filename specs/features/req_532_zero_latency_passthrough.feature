@REQ-532 @product
Feature: Zero-Latency Passthrough Mode — Minimal-Processing Path for Competitive Use  @AC-532.1
  Scenario: Zero-latency mode disables smoothing and rate limiting
    Given zero-latency mode is activated for an axis
    When an input sample arrives
    Then the axis engine SHALL bypass smoothing and rate limiting stages entirely  @AC-532.2
  Scenario: Zero-latency mode is activatable via profile flag
    Given a profile with zero_latency: true for the pitch axis
    When the profile is applied
    Then the axis engine SHALL operate in zero-latency mode for the pitch axis  @AC-532.3
  Scenario: Essential safety processing is preserved in zero-latency mode
    Given the axis is in zero-latency mode
    When an out-of-range input value is received
    Then clamping and NaN guards SHALL still be applied to the output  @AC-532.4
  Scenario: Latency is measurably lower than standard mode
    Given latency benchmarks are run for both standard and zero-latency modes
    When measurements are taken over 10000 samples
    Then the p99 latency in zero-latency mode SHALL be lower than in standard mode
