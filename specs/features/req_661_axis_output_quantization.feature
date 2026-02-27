Feature: Axis Output Quantization
  As a flight simulation enthusiast
  I want the axis engine to support output quantization for legacy sims
  So that I can match the discrete input ranges required by older simulators

  Background:
    Given the OpenFlight service is running

  Scenario: Output can be quantized to N evenly spaced steps
    Given quantization is configured with N steps for an axis
    When the axis produces output
    Then the output is mapped to the nearest of N evenly spaced steps

  Scenario: Quantization is configurable per axis in profile
    When a profile is authored
    Then each axis entry supports an optional quantization step count field

  Scenario: Quantized output is clamped to valid range
    Given quantization is active for an axis
    When an extreme input value is processed
    Then the quantized output is clamped within the valid axis range

  Scenario: Quantization is applied as the last pipeline stage
    Given quantization and other pipeline stages are configured for an axis
    When the axis pipeline executes
    Then quantization is applied after all other pipeline stages
