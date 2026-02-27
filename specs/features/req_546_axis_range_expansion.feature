Feature: Axis Range Expansion
  As a flight simulation enthusiast
  I want the axis engine to support range expansion beyond physical device limits
  So that I can increase effective resolution or sensitivity for precision maneuvers

  Background:
    Given the OpenFlight service is running
    And a joystick axis "PITCH" is configured with range expansion factor 1.5

  Scenario: Range expansion multiplies axis output
    Given the physical axis reports a normalized value of 0.4
    When the axis pipeline processes the value with expansion factor 1.5
    Then the output value is 0.6

  Scenario: Expanded output is clamped to valid range
    Given the physical axis reports a normalized value of 0.8
    When the axis pipeline applies expansion factor 1.5
    Then the output value is clamped to 1.0 and does not overflow

  Scenario: Range expansion interacts correctly with deadzone and curves
    Given the axis has a center deadzone of 0.05 and a mild S-curve
    And range expansion factor is 1.4
    When a value inside the deadzone is processed
    Then the output is zero regardless of the expansion factor
    When a value outside the deadzone is processed
    Then expansion is applied after deadzone and curve processing
