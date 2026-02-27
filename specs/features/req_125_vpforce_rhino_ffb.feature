@REQ-125 @product
Feature: VPForce Rhino FFB joystick

  @AC-125.1
  Scenario: Rhino axis parsed at center
    Given a VPForce Rhino HID input report with both axes at the midpoint value
    When the report is parsed
    Then the X-axis output SHALL be 0.0
    And the Y-axis output SHALL be 0.0

  @AC-125.2
  Scenario: Rhino axis parsed at maximum X deflection
    Given a VPForce Rhino HID input report with the X-axis at its maximum raw value
    When the report is parsed
    Then the X-axis output SHALL be approximately 1.0 (within 1e-3)
    And the Y-axis output SHALL be 0.0

  @AC-125.3
  Scenario: Rhino axis parsed at maximum Y deflection
    Given a VPForce Rhino HID input report with the Y-axis at its maximum raw value
    When the report is parsed
    Then the Y-axis output SHALL be approximately 1.0 (within 1e-3)
    And the X-axis output SHALL be 0.0

  @AC-125.4
  Scenario: Sine wave effect on X-axis
    Given a VPForce Rhino FFB engine
    When a sine wave effect is configured on the X-axis with frequency 10 Hz and amplitude 0.5
    Then the effect output SHALL oscillate between -0.5 and 0.5
    And the period SHALL be approximately 100 ms

  @AC-125.5
  Scenario: Constant force magnitude calculation
    Given a VPForce Rhino FFB engine
    When a constant force effect with magnitude 0.75 is applied on the X-axis
    Then the torque report sent to the device SHALL encode a value proportional to 0.75
    And the sign SHALL reflect the configured direction

  @AC-125.6
  Scenario: Spring and friction effects combined
    Given a VPForce Rhino FFB engine with both a spring effect and a friction effect active
    When the stick is displaced 0.4 units from centre at a velocity of 0.2 units/s
    Then the total force output SHALL be the sum of the spring restoring force and the friction opposing force
    And neither effect SHALL interfere with the other

  @AC-125.7
  Scenario: Any byte sequence processed without panic
    Given a VPForce Rhino HID report parser
    When it receives an arbitrary byte sequence of any length
    Then the parser SHALL return either a valid result or an error
    And it SHALL NOT panic under any input

  @AC-125.8
  Scenario: Axes always within valid bounds
    Given a VPForce Rhino HID report parser processing any valid input report
    When the axes are extracted
    Then both X and Y axis values SHALL be within [-1.0, 1.0]
