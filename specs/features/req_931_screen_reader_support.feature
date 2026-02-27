Feature: Screen Reader Support
  As a flight simulation enthusiast
  I want screen reader support
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: All UI elements have accessible labels compatible with screen readers
    Given the system is configured for screen reader support
    When the feature is exercised
    Then all UI elements have accessible labels compatible with screen readers

  Scenario: Focus order follows logical navigation flow through settings panels
    Given the system is configured for screen reader support
    When the feature is exercised
    Then focus order follows logical navigation flow through settings panels

  Scenario: Dynamic content changes announce updates to assistive technology
    Given the system is configured for screen reader support
    When the feature is exercised
    Then dynamic content changes announce updates to assistive technology

  Scenario: Screen reader compatibility is tested with NVDA and Windows Narrator
    Given the system is configured for screen reader support
    When the feature is exercised
    Then screen reader compatibility is tested with NVDA and Windows Narrator
