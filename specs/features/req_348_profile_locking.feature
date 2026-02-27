@REQ-348 @product
Feature: Profile Locking  @AC-348.1
  Scenario: Profile can be locked to prevent accidental changes
    Given an unlocked profile is active
    When the user locks the profile via CLI
    Then any subsequent edit attempt SHALL be rejected with a locked-profile error  @AC-348.2
  Scenario: Locked profile shows read-only indicator in CLI
    Given a profile is locked
    When the user runs "flightctl profile list"
    Then the locked profile SHALL be shown with a read-only indicator in the output  @AC-348.3
  Scenario: Lock/unlock requires explicit confirmation
    Given the user issues a lock or unlock command for a profile
    When the command is executed without a confirmation flag
    Then the CLI SHALL prompt for explicit confirmation before proceeding  @AC-348.4
  Scenario: Locked profile cannot be overwritten by auto-save
    Given a profile is locked and auto-save is enabled
    When an auto-save event is triggered for that profile
    Then the service SHALL skip the auto-save and log that the profile is locked  @AC-348.5
  Scenario: Lock state is persisted in profile metadata
    Given a profile is locked and the service is restarted
    When the service reloads the profile from disk
    Then the profile SHALL remain locked after the restart  @AC-348.6
  Scenario: Admin unlock bypasses user lock
    Given a profile is locked by a user
    When an administrator issues an unlock command with admin credentials
    Then the service SHALL unlock the profile regardless of the user lock state
