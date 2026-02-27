@REQ-1021
Feature: FFB Effect Chaining
  @AC-1021.1
  Scenario: Multiple FFB effects can be chained into a composite effect
    Given the system is configured for REQ-1021
    When the feature condition is met
    Then multiple ffb effects can be chained into a composite effect

  @AC-1021.2
  Scenario: Chain order determines effect composition priority
    Given the system is configured for REQ-1021
    When the feature condition is met
    Then chain order determines effect composition priority

  @AC-1021.3
  Scenario: Chained effects respect the overall FFB safety envelope
    Given the system is configured for REQ-1021
    When the feature condition is met
    Then chained effects respect the overall ffb safety envelope

  @AC-1021.4
  Scenario: Effect chain can be modified at runtime without interrupting playback
    Given the system is configured for REQ-1021
    When the feature condition is met
    Then effect chain can be modified at runtime without interrupting playback
