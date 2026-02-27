Feature: Notification Preferences
  As a flight simulation enthusiast
  I want notification preferences
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: User can configure notification verbosity from silent to verbose
    Given the system is configured for notification preferences
    When the feature is exercised
    Then user can configure notification verbosity from silent to verbose

  Scenario: Per-category notification settings allow selective muting
    Given the system is configured for notification preferences
    When the feature is exercised
    Then per-category notification settings allow selective muting

  Scenario: Notification preferences persist across service restarts
    Given the system is configured for notification preferences
    When the feature is exercised
    Then notification preferences persist across service restarts

  Scenario: Do-not-disturb mode suppresses all non-critical notifications
    Given the system is configured for notification preferences
    When the feature is exercised
    Then do-not-disturb mode suppresses all non-critical notifications
