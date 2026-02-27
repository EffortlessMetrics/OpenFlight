Feature: High Contrast Mode
  As a flight simulation enthusiast
  I want high contrast mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: UI provides high contrast theme meeting WCAG 2.1 AA contrast ratios
    Given the system is configured for high contrast mode
    When the feature is exercised
    Then uI provides high contrast theme meeting WCAG 2.1 AA contrast ratios

  Scenario: High contrast mode is selectable from settings without restart
    Given the system is configured for high contrast mode
    When the feature is exercised
    Then high contrast mode is selectable from settings without restart

  Scenario: UI automatically detects OS high contrast setting and adapts
    Given the system is configured for high contrast mode
    When the feature is exercised
    Then uI automatically detects OS high contrast setting and adapts

  Scenario: All interactive elements remain visible and distinguishable in high contrast
    Given the system is configured for high contrast mode
    When the feature is exercised
    Then all interactive elements remain visible and distinguishable in high contrast
