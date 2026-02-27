@REQ-416 @product
Feature: Axis Saturation Protection — Clamp Integrating Effects at Saturation

  @AC-416.1
  Scenario: Trim integrator is clamped when axis is saturated for more than 500 ms
    Given an axis with a trim integrator
    When the axis output reaches saturation and remains there for over 500 ms
    Then the trim integrator SHALL be clamped

  @AC-416.2
  Scenario: PID integral term is zeroed when saturation is detected
    Given an axis with a PID controller
    When saturation is detected
    Then the PID integral term SHALL be zeroed

  @AC-416.3
  Scenario: Saturation is defined as output reaching 0.95 or above |1.0|
    Given any axis output value
    When the absolute value is 0.95 or greater
    Then the axis SHALL be considered saturated

  @AC-416.4
  Scenario: Saturation recovery resets within one tick when axis moves off the limit
    Given an axis in saturation state
    When the axis value moves below the saturation threshold
    Then the saturation flag SHALL be cleared within one RT tick

  @AC-416.5
  Scenario: Saturation events are logged at WARN level with axis ID and duration
    Given an axis entering saturation
    When a saturation event occurs
    Then a WARN-level log entry SHALL be emitted containing the axis ID and saturation duration

  @AC-416.6
  Scenario: Property test — saturation state never causes output oscillation at ±1.0
    Given any sequence of saturating inputs
    When saturation protection is active
    Then the output SHALL never oscillate between +1.0 and -1.0 due to integrator windup
