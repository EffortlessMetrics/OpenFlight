@REQ-694
Feature: Axis Value Clamping
  @AC-694.1
  Scenario: Output values are clamped to configured min/max range
    Given the system is configured for REQ-694
    When the feature condition is met
    Then output values are clamped to configured min/max range

  @AC-694.2
  Scenario: Clamp range is configurable per axis
    Given the system is configured for REQ-694
    When the feature condition is met
    Then clamp range is configurable per axis

  @AC-694.3
  Scenario: Values outside clamp range are reported as metric events
    Given the system is configured for REQ-694
    When the feature condition is met
    Then values outside clamp range are reported as metric events

  @AC-694.4
  Scenario: Clamping operates as the final pipeline stage before output
    Given the system is configured for REQ-694
    When the feature condition is met
    Then clamping operates as the final pipeline stage before output
