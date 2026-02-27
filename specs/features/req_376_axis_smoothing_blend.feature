@REQ-376 @product
Feature: Axis Value Smoothing Blend — Blend Between Raw and Smoothed Output

  @AC-376.1
  Scenario: Blend factor controls the mix between raw and smoothed output
    Given an axis with a blend factor configured between 0.0 and 1.0
    When the axis processes an input value
    Then the output SHALL be a linear mix of raw and smoothed values weighted by the blend factor

  @AC-376.2
  Scenario: Blend factor is configurable per axis in the profile
    Given a profile with multiple axes each specifying different blend factors
    When the profile is loaded
    Then each axis SHALL use its configured blend factor

  @AC-376.3
  Scenario: Blend factor of 0.0 passes raw input unchanged
    Given an axis with blend factor set to 0.0
    When an input value is processed
    Then the output SHALL equal the raw input value

  @AC-376.4
  Scenario: Blend factor of 1.0 produces fully smoothed output
    Given an axis with blend factor set to 1.0
    When an input value is processed
    Then the output SHALL equal the fully smoothed value

  @AC-376.5
  Scenario: Property test — blended output is within bounds of raw and smoothed
    Given an axis with an arbitrary blend factor in [0.0, 1.0]
    When arbitrary input values are processed via property testing
    Then every blended output SHALL be within [min(raw, smoothed), max(raw, smoothed)]

  @AC-376.6
  Scenario: No allocation occurs on the RT thread during blend computation
    Given an axis with smoothing blend configured
    When the blend computation runs on the RT thread
    Then no heap allocation SHALL occur
