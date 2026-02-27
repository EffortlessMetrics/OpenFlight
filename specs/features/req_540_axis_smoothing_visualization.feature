Feature: Axis Smoothing Visualization Data
  As a flight simulation enthusiast
  I want axis diagnostics to include raw, smoothed, and output values
  So that I can tune my smoothing configuration visually

  Background:
    Given the OpenFlight service is running
    And at least one axis is active and receiving input

  Scenario: Diagnostics include raw, smoothed, and output values
    When I request axis diagnostics for "PITCH"
    Then the response includes raw input, smoothed value, and final output value
    And all three values are expressed as normalized floats

  Scenario: Last N samples available via RPC
    Given the sample buffer size is configured to 64
    When I call the GetAxisSamples RPC for axis "ROLL"
    Then the response contains up to 64 recent sample triplets
    And each triplet contains raw, smoothed, and output values

  Scenario: Visualization data exported as JSON
    When I run "flightctl axis samples PITCH --format json"
    Then the output is valid JSON
    And the JSON contains an array of sample objects with "raw", "smoothed", and "output" fields
