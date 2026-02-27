Feature: Profile Auto-Backup
  As a flight simulation enthusiast
  I want profile auto-backup
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Profiles are automatically backed up at a configurable interval
    Given the system is configured for profile auto-backup
    When the feature is exercised
    Then profiles are automatically backed up at a configurable interval

  Scenario: Backup rotation retains the most recent N backups per profile
    Given the system is configured for profile auto-backup
    When the feature is exercised
    Then backup rotation retains the most recent N backups per profile

  Scenario: Auto-backup triggers before any destructive profile operation
    Given the system is configured for profile auto-backup
    When the feature is exercised
    Then auto-backup triggers before any destructive profile operation

  Scenario: Backup files can be restored through the CLI or service API
    Given the system is configured for profile auto-backup
    When the feature is exercised
    Then backup files can be restored through the CLI or service API
