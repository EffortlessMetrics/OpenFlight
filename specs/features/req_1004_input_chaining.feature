@REQ-1004
Feature: Input Chaining
  @AC-1004.1
  Scenario: Multiple input actions can be chained into a named sequence
    Given the system is configured for REQ-1004
    When the feature condition is met
    Then multiple input actions can be chained into a named sequence

  @AC-1004.2
  Scenario: Chain steps execute in order with configurable delays between steps
    Given the system is configured for REQ-1004
    When the feature condition is met
    Then chain steps execute in order with configurable delays between steps

  @AC-1004.3
  Scenario: Chain execution can be interrupted by a cancel action
    Given the system is configured for REQ-1004
    When the feature condition is met
    Then chain execution can be interrupted by a cancel action

  @AC-1004.4
  Scenario: Chain progress is reported via the event bus
    Given the system is configured for REQ-1004
    When the feature condition is met
    Then chain progress is reported via the event bus
