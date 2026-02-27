Feature: Profile Conditional Activation
  As a flight simulation enthusiast
  I want profiles to support conditional activation rules
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Conditional rules are supported
    Given a profile has conditional activation rules
    When the conditions are evaluated
    Then the profile activates only when conditions are met

  Scenario: Conditions reference sim state
    Given a condition references a sim state variable
    When the sim state changes
    Then the condition is re-evaluated against the new state

  Scenario: AND/OR logic supported
    Given multiple conditions are defined with AND/OR operators
    When the conditions are evaluated
    Then the combined logic is applied correctly

  Scenario: Evaluation does not impact RT latency
    Given conditional evaluation is active
    When the RT spine processes a tick
    Then latency remains within the p99 budget
