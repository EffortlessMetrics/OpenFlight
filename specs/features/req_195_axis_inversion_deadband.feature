@REQ-195 @product
Feature: Individual axis inversion and deadband configuration works correctly  @AC-195.1
  Scenario: Axis invert flag flips sign of output value
    Given an axis is configured with the invert flag enabled
    When the axis reports an input value of 0.8
    Then the processed output SHALL be -0.8  @AC-195.2
  Scenario: Deadband configured per axis as fractional range
    Given an axis has a deadband value of 0.1 applied
    When the axis configuration is loaded
    Then the deadband SHALL be stored and applied as a fraction of the full axis range between 0.0 and 1.0  @AC-195.3
  Scenario: Inputs within deadband mapped to zero
    Given an axis with a deadband of 0.1
    When the raw input value is 0.05
    Then the processed output SHALL be exactly 0.0  @AC-195.4
  Scenario: Inputs outside deadband rescaled to full range
    Given an axis with a deadband of 0.1
    When the raw input value is 1.0
    Then the processed output SHALL be 1.0 with the edge-of-deadband mapping to 0.0 and maximum input mapping to 1.0  @AC-195.5
  Scenario: Deadband and invert can be combined without order dependency
    Given an axis with both a deadband of 0.1 and the invert flag enabled
    When the raw input value is 0.6
    Then the output SHALL be the same whether inversion or deadband is applied first  @AC-195.6
  Scenario: Profile validation rejects out-of-range deadband values
    Given a profile with a deadband value of 0.7 for an axis
    When the profile is validated
    Then profile validation SHALL reject the deadband value and report an error for values outside the range 0.0 to 0.5
