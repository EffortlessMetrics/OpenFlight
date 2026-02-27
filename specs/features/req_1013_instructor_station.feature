@REQ-1013
Feature: Instructor Station
  @AC-1013.1
  Scenario: Instructor station can override student input controls remotely
    Given the system is configured for REQ-1013
    When the feature condition is met
    Then instructor station can override student input controls remotely

  @AC-1013.2
  Scenario: Override is indicated to the student via visual and haptic feedback
    Given the system is configured for REQ-1013
    When the feature condition is met
    Then override is indicated to the student via visual and haptic feedback

  @AC-1013.3
  Scenario: Instructor can freeze specific axes while allowing others
    Given the system is configured for REQ-1013
    When the feature condition is met
    Then instructor can freeze specific axes while allowing others

  @AC-1013.4
  Scenario: All instructor actions are logged for training review
    Given the system is configured for REQ-1013
    When the feature condition is met
    Then all instructor actions are logged for training review
