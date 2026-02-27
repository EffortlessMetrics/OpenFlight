@REQ-696
Feature: Axis Swap Pair Configuration
  @AC-696.1
  Scenario: Two axes can be swapped via profile configuration
    Given the system is configured for REQ-696
    When the feature condition is met
    Then two axes can be swapped via profile configuration

  @AC-696.2
  Scenario: Swap applies to processed output, not raw input
    Given the system is configured for REQ-696
    When the feature condition is met
    Then swap applies to processed output, not raw input

  @AC-696.3
  Scenario: Swap configuration is validated to prevent circular references
    Given the system is configured for REQ-696
    When the feature condition is met
    Then swap configuration is validated to prevent circular references

  @AC-696.4
  Scenario: Axis swap is useful for adapting profiles to different hardware
    Given the system is configured for REQ-696
    When the feature condition is met
    Then axis swap is useful for adapting profiles to different hardware
