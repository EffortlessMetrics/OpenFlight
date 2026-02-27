@REQ-713
Feature: Axis Multi-Device Merge
  @AC-713.1
  Scenario: Multiple devices can contribute to the same logical axis
    Given the system is configured for REQ-713
    When the feature condition is met
    Then multiple devices can contribute to the same logical axis

  @AC-713.2
  Scenario: Merge strategy is configurable as priority, sum, or average
    Given the system is configured for REQ-713
    When the feature condition is met
    Then merge strategy is configurable as priority, sum, or average

  @AC-713.3
  Scenario: Device contribution weights are configurable per device
    Given the system is configured for REQ-713
    When the feature condition is met
    Then device contribution weights are configurable per device

  @AC-713.4
  Scenario: Merge conflicts are resolved deterministically by priority order
    Given the system is configured for REQ-713
    When the feature condition is met
    Then merge conflicts are resolved deterministically by priority order
