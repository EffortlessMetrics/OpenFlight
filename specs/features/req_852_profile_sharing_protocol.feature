Feature: Profile Sharing Protocol
  As a flight simulation enthusiast
  I want profile sharing protocol
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Profiles can be exported as signed portable packages
    Given the system is configured for profile sharing protocol
    When the feature is exercised
    Then profiles can be exported as signed portable packages

  Scenario: Imported profiles are validated against a cryptographic signature
    Given the system is configured for profile sharing protocol
    When the feature is exercised
    Then imported profiles are validated against a cryptographic signature

  Scenario: Sharing protocol includes device compatibility metadata
    Given the system is configured for profile sharing protocol
    When the feature is exercised
    Then sharing protocol includes device compatibility metadata

  Scenario: Import rejects packages with mismatched or missing signatures
    Given the system is configured for profile sharing protocol
    When the feature is exercised
    Then import rejects packages with mismatched or missing signatures
