Feature: Button Mapping Persistence
  As a flight simulation enthusiast
  I want button mapping persistence
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Button assignments are saved per game and per device combination
    Given the system is configured for button mapping persistence
    When the feature is exercised
    Then button assignments are saved per game and per device combination

  Scenario: Persisted mappings survive service restarts and device reconnection
    Given the system is configured for button mapping persistence
    When the feature is exercised
    Then persisted mappings survive service restarts and device reconnection

  Scenario: Button mapping conflicts are detected and reported to the user
    Given the system is configured for button mapping persistence
    When the feature is exercised
    Then button mapping conflicts are detected and reported to the user

  Scenario: Bulk import and export of button mappings is supported
    Given the system is configured for button mapping persistence
    When the feature is exercised
    Then bulk import and export of button mappings is supported