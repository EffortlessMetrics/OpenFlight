Feature: Update Rollback
  As a flight simulation enthusiast
  I want update rollback
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Failed update automatically reverts to the previous working version
    Given the system is configured for update rollback
    When the feature is exercised
    Then failed update automatically reverts to the previous working version

  Scenario: Rollback preserves user configuration without modification
    Given the system is configured for update rollback
    When the feature is exercised
    Then rollback preserves user configuration without modification

  Scenario: Manual rollback command is available via CLI for user-initiated revert
    Given the system is configured for update rollback
    When the feature is exercised
    Then manual rollback command is available via CLI for user-initiated revert

  Scenario: Rollback event is logged with reason and prior version details
    Given the system is configured for update rollback
    When the feature is exercised
    Then rollback event is logged with reason and prior version details
