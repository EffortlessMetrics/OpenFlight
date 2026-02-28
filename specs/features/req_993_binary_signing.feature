Feature: Binary Signing
  As a flight simulation enthusiast
  I want binary signing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: All distributed binaries are code-signed with verified certificates
    Given the system is configured for binary signing
    When the feature is exercised
    Then all distributed binaries are code-signed with verified certificates

  Scenario: Signature verification is performed during installation and update
    Given the system is configured for binary signing
    When the feature is exercised
    Then signature verification is performed during installation and update

  Scenario: Signing key rotation is supported without breaking existing installations
    Given the system is configured for binary signing
    When the feature is exercised
    Then signing key rotation is supported without breaking existing installations

  Scenario: Unsigned or tampered binaries are rejected with clear error messaging
    Given the system is configured for binary signing
    When the feature is exercised
    Then unsigned or tampered binaries are rejected with clear error messaging