@REQ-1010
Feature: Multi-User Profiles
  @AC-1010.1
  Scenario: Multiple user profiles can exist on the same installation
    Given the system is configured for REQ-1010
    When the feature condition is met
    Then multiple user profiles can exist on the same installation

  @AC-1010.2
  Scenario: WHEN user switches THEN all mappings and settings SHALL change to that user profile
    Given the system is configured for REQ-1010
    When the feature condition is met
    Then when user switches then all mappings and settings shall change to that user profile

  @AC-1010.3
  Scenario: User profile selection is available via CLI and UI
    Given the system is configured for REQ-1010
    When the feature condition is met
    Then user profile selection is available via cli and ui

  @AC-1010.4
  Scenario: Per-user settings are isolated in separate configuration directories
    Given the system is configured for REQ-1010
    When the feature condition is met
    Then per-user settings are isolated in separate configuration directories
