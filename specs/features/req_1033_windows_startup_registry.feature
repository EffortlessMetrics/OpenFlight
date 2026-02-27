@REQ-1033
Feature: Windows Startup Registry
  @AC-1033.1
  Scenario: Auto-start on Windows login is configurable via CLI or UI
    Given the system is configured for REQ-1033
    When the feature condition is met
    Then auto-start on windows login is configurable via cli or ui

  @AC-1033.2
  Scenario: Startup entry is added to current user registry run key
    Given the system is configured for REQ-1033
    When the feature condition is met
    Then startup entry is added to current user registry run key

  @AC-1033.3
  Scenario: Disabling auto-start removes the registry entry cleanly
    Given the system is configured for REQ-1033
    When the feature condition is met
    Then disabling auto-start removes the registry entry cleanly

  @AC-1033.4
  Scenario: Startup configuration is verified during service health check
    Given the system is configured for REQ-1033
    When the feature condition is met
    Then startup configuration is verified during service health check
