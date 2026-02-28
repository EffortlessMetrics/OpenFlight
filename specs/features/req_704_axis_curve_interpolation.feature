@REQ-704
Feature: Axis Curve Interpolation
  @AC-704.1
  Scenario: Interpolation between control points uses configurable method
    Given the system is configured for REQ-704
    When the feature condition is met
    Then interpolation between control points uses configurable method

  @AC-704.2
  Scenario: Supported methods include linear, cubic, and Catmull-Rom
    Given the system is configured for REQ-704
    When the feature condition is met
    Then supported methods include linear, cubic, and catmull-rom

  @AC-704.3
  Scenario: Interpolation produces smooth output without discontinuities
    Given the system is configured for REQ-704
    When the feature condition is met
    Then interpolation produces smooth output without discontinuities

  @AC-704.4
  Scenario: Interpolation lookup table is precomputed for RT performance
    Given the system is configured for REQ-704
    When the feature condition is met
    Then interpolation lookup table is precomputed for rt performance
