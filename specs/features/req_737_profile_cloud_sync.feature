Feature: Profile Cloud Sync
  As a flight simulation enthusiast
  I want profiles to sync via cloud storage
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Sync to cloud backend
    Given a cloud storage backend is configured
    When a profile is saved
    Then it is synced to the cloud backend

  Scenario: Conflict resolution
    Given a sync conflict is detected
    When the conflict is resolved
    Then last-write-wins or manual merge is applied

  Scenario: Sync status visible in CLI
    Given cloud sync is enabled
    When I run the profile sync-status command
    Then the current sync status is displayed

  Scenario: Disable without data loss
    Given cloud sync is enabled
    When sync is disabled
    Then local profiles are preserved without data loss
