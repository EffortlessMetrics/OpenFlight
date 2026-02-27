@REQ-697
Feature: Axis Trim Adjustment
  @AC-697.1
  Scenario: Trim offset can be applied to shift the axis center point
    Given the system is configured for REQ-697
    When the feature condition is met
    Then trim offset can be applied to shift the axis center point

  @AC-697.2
  Scenario: Trim can be adjusted incrementally via bound buttons
    Given the system is configured for REQ-697
    When the feature condition is met
    Then trim can be adjusted incrementally via bound buttons

  @AC-697.3
  Scenario: Trim position is displayed in the axis status view
    Given the system is configured for REQ-697
    When the feature condition is met
    Then trim position is displayed in the axis status view

  @AC-697.4
  Scenario: Trim resets to zero on profile change unless persisted
    Given the system is configured for REQ-697
    When the feature condition is met
    Then trim resets to zero on profile change unless persisted
