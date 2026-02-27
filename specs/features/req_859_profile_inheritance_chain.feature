Feature: Profile Inheritance Chain
  As a flight simulation enthusiast
  I want profile inheritance chain
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Profiles support multi-level inheritance with override visibility
    Given the system is configured for profile inheritance chain
    When the feature is exercised
    Then profiles support multi-level inheritance with override visibility

  Scenario: Inheritance chain displays which level each setting originates from
    Given the system is configured for profile inheritance chain
    When the feature is exercised
    Then inheritance chain displays which level each setting originates from

  Scenario: Circular inheritance references are detected and rejected
    Given the system is configured for profile inheritance chain
    When the feature is exercised
    Then circular inheritance references are detected and rejected

  Scenario: Override at any level can be inspected and reverted independently
    Given the system is configured for profile inheritance chain
    When the feature is exercised
    Then override at any level can be inspected and reverted independently
