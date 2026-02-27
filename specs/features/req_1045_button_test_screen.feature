@REQ-1045
Feature: Button Test Screen
  @AC-1045.1
  Scenario: Built-in button test highlights active buttons in real-time
    Given the system is configured for REQ-1045
    When the feature condition is met
    Then built-in button test highlights active buttons in real-time

  @AC-1045.2
  Scenario: Test screen shows button state history with timestamps
    Given the system is configured for REQ-1045
    When the feature condition is met
    Then test screen shows button state history with timestamps

  @AC-1045.3
  Scenario: Button test displays both physical and mapped button identifiers
    Given the system is configured for REQ-1045
    When the feature condition is met
    Then button test displays both physical and mapped button identifiers

  @AC-1045.4
  Scenario: Test screen is accessible via CLI and UI
    Given the system is configured for REQ-1045
    When the feature condition is met
    Then test screen is accessible via cli and ui
