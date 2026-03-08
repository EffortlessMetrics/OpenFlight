@REQ-1056 @product @user-journey
Feature: Software update user journey
  As an OpenFlight user
  I want to check for, apply, and roll back software updates
  So that I stay current while having a safety net if an update causes problems

  @AC-1056.1
  Scenario: Check for available updates
    Given the OpenFlight service is running version "1.2.0"
    And the update channel is set to "stable"
    When the user triggers an update check via the CLI
    Then the updater SHALL query the configured update server
    And if version "1.3.0" is available it SHALL be reported with release notes
    And the update check result SHALL include file size and SHA-256 hash
    And a "update_available" event SHALL be emitted on the bus

  @AC-1056.2
  Scenario: Apply an update with service restart
    Given an update to version "1.3.0" has been downloaded and verified
    When the user confirms the update via the CLI
    Then the updater SHALL back up the current installation to a rollback directory
    And the update SHALL be applied atomically
    And the service SHALL restart with the new version
    And a "update_applied" event SHALL be emitted with old and new version numbers

  @AC-1056.3
  Scenario: Rollback update restores previous version
    Given version "1.3.0" was installed and the rollback directory contains version "1.2.0"
    When the user triggers a rollback via the CLI
    Then the updater SHALL restore the backed-up version "1.2.0" files
    And the service SHALL restart with the restored version
    And a "update_rolled_back" event SHALL be emitted with the restored version
    And all user profiles and configuration SHALL be preserved across the rollback
