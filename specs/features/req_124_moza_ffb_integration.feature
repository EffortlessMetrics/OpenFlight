@REQ-124 @product
Feature: MOZA R9 FFB base integration

  @AC-124.1
  Scenario: MOZA R9 axis parsed at center
    Given a MOZA R9 HID input report with both axes at the midpoint value
    When the report is parsed
    Then the X-axis output SHALL be 0.0
    And the Y-axis output SHALL be 0.0

  @AC-124.2
  Scenario: MOZA R9 axis parsed at maximum deflection
    Given a MOZA R9 HID input report with the X-axis at its maximum raw value
    When the report is parsed
    Then the X-axis output SHALL be approximately 1.0 (within 1e-3)

  @AC-124.3
  Scenario: MOZA FFB constant force direction matches input
    Given a MOZA FFB engine
    When a constant force effect is created with direction vector (0.5, -0.5)
    Then the torque output direction SHALL match (0.5, -0.5)
    And the magnitude SHALL be proportional to the configured strength

  @AC-124.4
  Scenario: MOZA FFB spring centering with configurable stiffness
    Given a MOZA FFB engine with a spring effect at stiffness 0.8
    When the stick is displaced 0.5 units from centre
    Then the restoring force SHALL be proportional to displacement × stiffness
    And increasing stiffness SHALL increase the restoring force

  @AC-124.5
  Scenario: MOZA FFB friction effect applied
    Given a MOZA FFB engine with a friction effect at coefficient 0.3
    When the stick velocity is non-zero
    Then a friction force SHALL oppose the direction of motion
    And a zero-velocity input SHALL produce zero friction force

  @AC-124.6
  Scenario: Short HID report rejected gracefully
    Given a HID input buffer shorter than the expected MOZA R9 report length
    When the report parser is called
    Then the result SHALL be an error indicating a short buffer
    And no axis state SHALL be emitted

  @AC-124.7
  Scenario: Axis values clamped to [-1.0, 1.0]
    Given a MOZA R9 HID report with raw axis bytes encoding a value beyond the nominal range
    When the report is parsed
    Then the resulting axis value SHALL be clamped to the range [-1.0, 1.0]

  @AC-124.8
  Scenario: Device identified by VID/PID
    Given a connected HID device with MOZA R9 vendor ID and product ID
    When the device enumeration runs
    Then the device SHALL be identified as a MOZA R9 FFB base
    And it SHALL be registered with the FFB engine
