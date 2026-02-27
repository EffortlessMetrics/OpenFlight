Feature: Update Integrity
  As a flight simulation enthusiast
  I want update integrity
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: All update payloads are signed with Ed25519 signatures
    Given the system is configured for update integrity
    When the feature is exercised
    Then all update payloads are signed with Ed25519 signatures

  Scenario: Signature verification occurs before any update files are extracted
    Given the system is configured for update integrity
    When the feature is exercised
    Then signature verification occurs before any update files are extracted

  Scenario: Update manifest includes SHA-256 hashes for every contained file
    Given the system is configured for update integrity
    When the feature is exercised
    Then update manifest includes SHA-256 hashes for every contained file

  Scenario: Tampered or unsigned payloads are rejected with descriptive error
    Given the system is configured for update integrity
    When the feature is exercised
    Then tampered or unsigned payloads are rejected with descriptive error
