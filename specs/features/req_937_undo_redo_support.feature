Feature: Undo Redo Support
  As a flight simulation enthusiast
  I want undo redo support
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configuration changes support undo operation via Ctrl+Z
    Given the system is configured for undo redo support
    When the feature is exercised
    Then configuration changes support undo operation via Ctrl+Z

  Scenario: Redo operation restores undone changes via Ctrl+Y
    Given the system is configured for undo redo support
    When the feature is exercised
    Then redo operation restores undone changes via Ctrl+Y

  Scenario: Undo history persists within the current editing session
    Given the system is configured for undo redo support
    When the feature is exercised
    Then undo history persists within the current editing session

  Scenario: Undo stack depth is bounded to prevent excessive memory usage
    Given the system is configured for undo redo support
    When the feature is exercised
    Then undo stack depth is bounded to prevent excessive memory usage
