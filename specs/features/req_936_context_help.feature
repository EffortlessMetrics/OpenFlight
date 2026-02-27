Feature: Context Help
  As a flight simulation enthusiast
  I want context help
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Each settings panel includes contextual help accessible via help icon
    Given the system is configured for context help
    When the feature is exercised
    Then each settings panel includes contextual help accessible via help icon

  Scenario: Help content explains the setting purpose and recommended values
    Given the system is configured for context help
    When the feature is exercised
    Then help content explains the setting purpose and recommended values

  Scenario: Help text is searchable from a global help search function
    Given the system is configured for context help
    When the feature is exercised
    Then help text is searchable from a global help search function

  Scenario: Help content links to relevant online documentation when available
    Given the system is configured for context help
    When the feature is exercised
    Then help content links to relevant online documentation when available
