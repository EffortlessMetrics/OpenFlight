@REQ-705
Feature: Axis Curve Smoothness Validation
  @AC-705.1
  Scenario: Curve smoothness is validated when profile is loaded
    Given the system is configured for REQ-705
    When the feature condition is met
    Then curve smoothness is validated when profile is loaded

  @AC-705.2
  Scenario: Discontinuities in curve slope generate warnings
    Given the system is configured for REQ-705
    When the feature condition is met
    Then discontinuities in curve slope generate warnings

  @AC-705.3
  Scenario: Maximum slope change between adjacent segments is configurable
    Given the system is configured for REQ-705
    When the feature condition is met
    Then maximum slope change between adjacent segments is configurable

  @AC-705.4
  Scenario: Validation results are included in profile lint output
    Given the system is configured for REQ-705
    When the feature condition is met
    Then validation results are included in profile lint output
