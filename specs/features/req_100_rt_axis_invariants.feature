@REQ-100
Feature: RT axis processing pipeline invariants

  @AC-100.1
  Scenario: Deadzone center snaps to zero output
    Given a DeadzoneNode with a configured threshold
    When the input value is within the deadzone band
    Then the output SHALL be exactly 0.0

  @AC-100.2
  Scenario: Expo=0.0 produces linear (identity) response
    Given a CurveNode with expo set to 0.0
    When any input value in [-1.0, 1.0] is processed
    Then the output SHALL equal the input within floating-point tolerance

  @AC-100.3
  Scenario: Curve with any expo is monotone
    Given a CurveNode with any expo in [-1.0, 1.0]
    When two inputs a and b where a is less than or equal to b are processed
    Then the output for a SHALL be less than or equal to the output for b

  @AC-100.4
  Scenario: Pipeline output is always clamped to [-1.0, 1.0]
    Given an axis engine with any pipeline configuration
    When any input value is processed through the engine
    Then the output SHALL be within the range [-1.0, 1.0]

  @AC-100.5
  Scenario: SlewNode converges toward target at the configured rate
    Given a SlewNode compiled from a valid slew configuration
    When the input changes step-wise
    Then the output SHALL converge toward the target bounded by the configured slew rate
