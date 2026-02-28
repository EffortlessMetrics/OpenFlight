Feature: Profile Conflict Detection
  As a flight simulation enthusiast
  I want profile conflict detection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Conflicting axis assignments across profiles are detected at load time
    Given the system is configured for profile conflict detection
    When the feature is exercised
    Then conflicting axis assignments across profiles are detected at load time

  Scenario: Conflict report identifies the specific axes and profiles involved
    Given the system is configured for profile conflict detection
    When the feature is exercised
    Then conflict report identifies the specific axes and profiles involved

  Scenario: Service emits a warning event when conflicts are detected
    Given the system is configured for profile conflict detection
    When the feature is exercised
    Then service emits a warning event when conflicts are detected

  Scenario: User can resolve conflicts interactively or via a priority rule
    Given the system is configured for profile conflict detection
    When the feature is exercised
    Then user can resolve conflicts interactively or via a priority rule
