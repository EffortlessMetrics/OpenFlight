Feature: Axis Multi-Curve Blending
  As a flight simulation enthusiast
  I want axis curves to support blending between two curves
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Blending between two curves
    Given two axis curves are defined
    When blending is enabled
    Then the output blends between the two curves

  Scenario: Blend ratio configurable
    Given curve blending is active
    When the blend ratio is set to a value between 0.0 and 1.0
    Then the output reflects the configured ratio

  Scenario: No heap allocation during blending
    Given curve blending is active on the RT path
    When the blend is computed
    Then no heap allocation occurs

  Scenario: Smooth transitions
    Given the blend ratio is changing over time
    When the ratio transitions
    Then the output is smooth with no discontinuities
