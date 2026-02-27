@REQ-403 @product
Feature: Axis Blend Value Smoothing — Blend Raw and Filtered Values Per-Tick

  @AC-403.1
  Scenario: Blend factor is configurable per axis
    Given an axis with a configured blend factor of 0.7
    When the axis processes a tick
    Then the blend factor SHALL be applied individually to that axis

  @AC-403.2
  Scenario: Blend is applied as the last step after all other filters
    Given an axis with curve, deadzone, and blend filters configured
    When a tick is processed
    Then the blend SHALL be the final operation in the filter chain

  @AC-403.3
  Scenario: Property test — blend output is always between raw and filtered
    Given an axis with any valid blend factor between 0.0 and 1.0
    And raw and filtered values within [-1.0, 1.0]
    When blend is applied
    Then the output SHALL always be between the raw and filtered values

  @AC-403.4
  Scenario: Blend of 0.5 produces geometric mean of raw and filtered
    Given a raw value of 0.4 and a filtered value of 0.8
    And a blend factor of 0.5
    When the blend is computed
    Then the output SHALL equal the geometric mean of 0.4 and 0.8

  @AC-403.5
  Scenario: Blend factor is validated to be in [0.0, 1.0]
    Given a blend factor outside the range [0.0, 1.0]
    When the configuration is loaded
    Then a validation error SHALL be returned

  @AC-403.6
  Scenario: Zero allocation on RT thread during blend
    Given blend is configured for an axis
    When a tick is processed on the RT thread
    Then no heap allocation SHALL occur during the blend operation
