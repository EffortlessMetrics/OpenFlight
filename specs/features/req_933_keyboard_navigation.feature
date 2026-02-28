Feature: Keyboard Navigation
  As a flight simulation enthusiast
  I want keyboard navigation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: All UI functionality is accessible via keyboard without mouse
    Given the system is configured for keyboard navigation
    When the feature is exercised
    Then all UI functionality is accessible via keyboard without mouse

  Scenario: Tab order follows logical flow and is consistent across panels
    Given the system is configured for keyboard navigation
    When the feature is exercised
    Then tab order follows logical flow and is consistent across panels

  Scenario: Keyboard shortcuts are documented and discoverable in-app
    Given the system is configured for keyboard navigation
    When the feature is exercised
    Then keyboard shortcuts are documented and discoverable in-app

  Scenario: Focus indicators are clearly visible on all interactive elements
    Given the system is configured for keyboard navigation
    When the feature is exercised
    Then focus indicators are clearly visible on all interactive elements
