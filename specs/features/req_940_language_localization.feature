Feature: Language Localization Framework
  As a flight simulation enthusiast
  I want language localization framework
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: UI strings are externalized in resource files for translation
    Given the system is configured for language localization framework
    When the feature is exercised
    Then uI strings are externalized in resource files for translation

  Scenario: Language selection is configurable in settings with immediate effect
    Given the system is configured for language localization framework
    When the feature is exercised
    Then language selection is configurable in settings with immediate effect

  Scenario: Fallback to English occurs for untranslated strings
    Given the system is configured for language localization framework
    When the feature is exercised
    Then fallback to English occurs for untranslated strings

  Scenario: Date, time, and number formatting respects locale conventions
    Given the system is configured for language localization framework
    When the feature is exercised
    Then date, time, and number formatting respects locale conventions
