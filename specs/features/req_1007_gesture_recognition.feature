@REQ-1007
Feature: Gesture Recognition
  @AC-1007.1
  Scenario: Movement patterns on multi-axis devices can be recognized as gestures
    Given the system is configured for REQ-1007
    When the feature condition is met
    Then movement patterns on multi-axis devices can be recognized as gestures

  @AC-1007.2
  Scenario: Gesture templates are defined in profile with tolerance parameters
    Given the system is configured for REQ-1007
    When the feature condition is met
    Then gesture templates are defined in profile with tolerance parameters

  @AC-1007.3
  Scenario: WHEN a gesture is recognized THEN the configured action SHALL trigger
    Given the system is configured for REQ-1007
    When the feature condition is met
    Then when a gesture is recognized then the configured action shall trigger

  @AC-1007.4
  Scenario: Gesture recognition operates within RT processing budget
    Given the system is configured for REQ-1007
    When the feature condition is met
    Then gesture recognition operates within rt processing budget
