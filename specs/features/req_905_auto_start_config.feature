Feature: Auto-Start Configuration
  As a flight simulation enthusiast
  I want auto-start configuration
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Service starts automatically on user login when auto-start is enabled
    Given the system is configured for auto-start configuration
    When the feature is exercised
    Then service starts automatically on user login when auto-start is enabled

  Scenario: Auto-start can be toggled via CLI command without manual config editing
    Given the system is configured for auto-start configuration
    When the feature is exercised
    Then auto-start can be toggled via CLI command without manual config editing

  Scenario: Windows auto-start uses Task Scheduler with user-level privileges
    Given the system is configured for auto-start configuration
    When the feature is exercised
    Then windows auto-start uses Task Scheduler with user-level privileges

  Scenario: Linux auto-start uses systemd user unit with correct dependencies
    Given the system is configured for auto-start configuration
    When the feature is exercised
    Then linux auto-start uses systemd user unit with correct dependencies
