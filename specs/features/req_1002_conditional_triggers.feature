@REQ-1002
Feature: Conditional Triggers
  @AC-1002.1
  Scenario: Triggers can be defined with conditions based on sim state variables
    Given the system is configured for REQ-1002
    When the feature condition is met
    Then triggers can be defined with conditions based on sim state variables

  @AC-1002.2
  Scenario: WHEN condition evaluates to true THEN the configured action SHALL execute
    Given the system is configured for REQ-1002
    When the feature condition is met
    Then when condition evaluates to true then the configured action shall execute

  @AC-1002.3
  Scenario: Multiple conditions can be combined with AND/OR logic
    Given the system is configured for REQ-1002
    When the feature condition is met
    Then multiple conditions can be combined with and/or logic

  @AC-1002.4
  Scenario: Invalid condition expressions are rejected at profile load time
    Given the system is configured for REQ-1002
    When the feature condition is met
    Then invalid condition expressions are rejected at profile load time
