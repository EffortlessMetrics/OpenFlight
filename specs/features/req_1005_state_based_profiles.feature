@REQ-1005
Feature: State-Based Profiles
  @AC-1005.1
  Scenario: Profile auto-switching rules can reference sim state conditions
    Given the system is configured for REQ-1005
    When the feature condition is met
    Then profile auto-switching rules can reference sim state conditions

  @AC-1005.2
  Scenario: WHEN flight phase changes THEN the matching profile SHALL activate
    Given the system is configured for REQ-1005
    When the feature condition is met
    Then when flight phase changes then the matching profile shall activate

  @AC-1005.3
  Scenario: State conditions support comparison operators for numeric sim variables
    Given the system is configured for REQ-1005
    When the feature condition is met
    Then state conditions support comparison operators for numeric sim variables

  @AC-1005.4
  Scenario: Profile switch hysteresis prevents rapid toggling between profiles
    Given the system is configured for REQ-1005
    When the feature condition is met
    Then profile switch hysteresis prevents rapid toggling between profiles
