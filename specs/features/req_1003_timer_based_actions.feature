@REQ-1003
Feature: Timer-Based Actions
  @AC-1003.1
  Scenario: Actions can be scheduled to execute after a configurable delay
    Given the system is configured for REQ-1003
    When the feature condition is met
    Then actions can be scheduled to execute after a configurable delay

  @AC-1003.2
  Scenario: Repeating timer actions execute at configurable intervals
    Given the system is configured for REQ-1003
    When the feature condition is met
    Then repeating timer actions execute at configurable intervals

  @AC-1003.3
  Scenario: Timer actions can be cancelled before execution
    Given the system is configured for REQ-1003
    When the feature condition is met
    Then timer actions can be cancelled before execution

  @AC-1003.4
  Scenario: Timer precision is within 10ms of configured interval
    Given the system is configured for REQ-1003
    When the feature condition is met
    Then timer precision is within 10ms of configured interval
