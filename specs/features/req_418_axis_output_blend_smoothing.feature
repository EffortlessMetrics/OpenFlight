@REQ-418 @product
Feature: Axis Output Blend Smoothing — Configurable Blend Between Raw and Filtered Output

  @AC-418.1
  Scenario: Blend factor 0.0 produces raw output only
    Given an axis with blend_factor set to 0.0
    When a tick is processed
    Then the output SHALL equal the raw (unfiltered) axis value

  @AC-418.2
  Scenario: Blend factor 1.0 produces fully filtered output only
    Given an axis with blend_factor set to 1.0
    When a tick is processed
    Then the output SHALL equal the fully filtered axis value

  @AC-418.3
  Scenario: Blend factor 0.5 averages raw and filtered
    Given an axis with blend_factor set to 0.5
    When a tick is processed
    Then the output SHALL equal the arithmetic average of raw and filtered values

  @AC-418.4
  Scenario: Property test — blend output is always between raw and filtered values
    Given any blend_factor in [0.0, 1.0] and any valid raw and filtered axis values
    When the blend is computed
    Then the output SHALL be between (or equal to) the raw and filtered values

  @AC-418.5
  Scenario: Blend is applied as the final post-processing step
    Given an axis pipeline with curve, deadzone, rate limiter, and blend configured
    When a tick is processed
    Then the blend SHALL be applied last, after all other processing stages

  @AC-418.6
  Scenario: Blend factor is configurable per axis in the profile
    Given a profile with different blend_factor values per axis
    When the profile is loaded
    Then each axis SHALL use its own configured blend_factor independently
