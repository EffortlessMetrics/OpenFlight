Feature: Progress Indicators
  As a flight simulation enthusiast
  I want progress indicators
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Long operations display progress bar with estimated time remaining
    Given the system is configured for progress indicators
    When the feature is exercised
    Then long operations display progress bar with estimated time remaining

  Scenario: Indeterminate operations show activity spinner to indicate work in progress
    Given the system is configured for progress indicators
    When the feature is exercised
    Then indeterminate operations show activity spinner to indicate work in progress

  Scenario: Progress state is reported via IPC for headless monitoring
    Given the system is configured for progress indicators
    When the feature is exercised
    Then progress state is reported via IPC for headless monitoring

  Scenario: Cancel button is available for interruptible long-running operations
    Given the system is configured for progress indicators
    When the feature is exercised
    Then cancel button is available for interruptible long-running operations
