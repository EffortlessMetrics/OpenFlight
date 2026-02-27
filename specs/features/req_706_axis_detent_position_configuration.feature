@REQ-706
Feature: Axis Detent Position Configuration
  @AC-706.1
  Scenario: Virtual detent positions can be defined along the axis range
    Given the system is configured for REQ-706
    When the feature condition is met
    Then virtual detent positions can be defined along the axis range

  @AC-706.2
  Scenario: Each detent has configurable position and width
    Given the system is configured for REQ-706
    When the feature condition is met
    Then each detent has configurable position and width

  @AC-706.3
  Scenario: Multiple detents are supported per axis
    Given the system is configured for REQ-706
    When the feature condition is met
    Then multiple detents are supported per axis

  @AC-706.4
  Scenario: Detent configuration is validated for non-overlapping positions
    Given the system is configured for REQ-706
    When the feature condition is met
    Then detent configuration is validated for non-overlapping positions
