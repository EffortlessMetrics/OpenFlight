@REQ-689
Feature: Service Update Rollback
  @AC-689.1
  Scenario: If updated service fails health check within 60s, rollback is triggered
    Given the system is configured for REQ-689
    When the feature condition is met
    Then if updated service fails health check within 60s, rollback is triggered

  @AC-689.2
  Scenario: Previous version binary is preserved in a rollback slot
    Given the system is configured for REQ-689
    When the feature condition is met
    Then previous version binary is preserved in a rollback slot

  @AC-689.3
  Scenario: Rollback event is logged and surfaced in CLI
    Given the system is configured for REQ-689
    When the feature condition is met
    Then rollback event is logged and surfaced in cli

  @AC-689.4
  Scenario: Rollback policy is configurable in update config
    Given the system is configured for REQ-689
    When the feature condition is met
    Then rollback policy is configurable in update config
