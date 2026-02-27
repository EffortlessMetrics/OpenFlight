@REQ-1036
Feature: Deep Linking
  @AC-1036.1
  Scenario: URL scheme openflight:// is registered for command dispatch
    Given the system is configured for REQ-1036
    When the feature condition is met
    Then url scheme openflight:// is registered for command dispatch

  @AC-1036.2
  Scenario: Deep links can trigger profile loads, device actions, and navigation
    Given the system is configured for REQ-1036
    When the feature condition is met
    Then deep links can trigger profile loads, device actions, and navigation

  @AC-1036.3
  Scenario: Deep link parameters are validated before execution
    Given the system is configured for REQ-1036
    When the feature condition is met
    Then deep link parameters are validated before execution

  @AC-1036.4
  Scenario: Malformed deep links are rejected with descriptive error logging
    Given the system is configured for REQ-1036
    When the feature condition is met
    Then malformed deep links are rejected with descriptive error logging
