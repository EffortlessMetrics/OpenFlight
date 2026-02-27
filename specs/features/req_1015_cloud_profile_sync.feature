@REQ-1015
Feature: Cloud Profile Sync
  @AC-1015.1
  Scenario: Profiles can be synced to cloud storage for cross-machine access
    Given the system is configured for REQ-1015
    When the feature condition is met
    Then profiles can be synced to cloud storage for cross-machine access

  @AC-1015.2
  Scenario: Cloud sync handles conflict resolution with last-write-wins or manual merge
    Given the system is configured for REQ-1015
    When the feature condition is met
    Then cloud sync handles conflict resolution with last-write-wins or manual merge

  @AC-1015.3
  Scenario: Sync status is visible in CLI and UI
    Given the system is configured for REQ-1015
    When the feature condition is met
    Then sync status is visible in cli and ui

  @AC-1015.4
  Scenario: Cloud sync operates with end-to-end encryption for profile data
    Given the system is configured for REQ-1015
    When the feature condition is met
    Then cloud sync operates with end-to-end encryption for profile data
