Feature: Profile Quick-Switch
  As a flight simulation enthusiast
  I want profile quick-switch
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Profiles can be switched via a configurable hotkey combination
    Given the system is configured for profile quick-switch
    When the feature is exercised
    Then profiles can be switched via a configurable hotkey combination

  Scenario: Quick-switch completes within 100ms without dropping input frames
    Given the system is configured for profile quick-switch
    When the feature is exercised
    Then quick-switch completes within 100ms without dropping input frames

  Scenario: An on-screen indicator confirms the active profile after switch
    Given the system is configured for profile quick-switch
    When the feature is exercised
    Then an on-screen indicator confirms the active profile after switch

  Scenario: Quick-switch cycles through a user-defined profile ring
    Given the system is configured for profile quick-switch
    When the feature is exercised
    Then quick-switch cycles through a user-defined profile ring
