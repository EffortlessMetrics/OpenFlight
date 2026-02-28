@REQ-692
Feature: Axis Filter Bypass Mode
  @AC-692.1
  Scenario: Individual filters can be bypassed without removing configuration
    Given the system is configured for REQ-692
    When the feature condition is met
    Then individual filters can be bypassed without removing configuration

  @AC-692.2
  Scenario: Bypass state is togglable at runtime via CLI
    Given the system is configured for REQ-692
    When the feature condition is met
    Then bypass state is togglable at runtime via cli

  @AC-692.3
  Scenario: Bypassed filters consume zero processing budget
    Given the system is configured for REQ-692
    When the feature condition is met
    Then bypassed filters consume zero processing budget

  @AC-692.4
  Scenario: Filter bypass state is persisted across service restarts
    Given the system is configured for REQ-692
    When the feature condition is met
    Then filter bypass state is persisted across service restarts
