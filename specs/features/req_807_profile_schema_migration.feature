Feature: Profile Schema Migration
  As a flight simulation enthusiast
  I want profile schema migration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Detect schema version from metadata
    Given the system is configured for profile schema migration
    When the feature is exercised
    Then profile loader detects schema version from profile metadata

  Scenario: Auto-migrate older schema versions
    Given the system is configured for profile schema migration
    When the feature is exercised
    Then profiles with older schema versions are migrated to current version automatically

  Scenario: Preserve all values without data loss
    Given the system is configured for profile schema migration
    When the feature is exercised
    Then migration preserves all user-configured values without data loss

  Scenario: Report detailed errors on migration failure
    Given the system is configured for profile schema migration
    When the feature is exercised
    Then migration failures produce a detailed error report identifying incompatible fields
