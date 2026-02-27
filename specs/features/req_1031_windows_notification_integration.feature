@REQ-1031
Feature: Windows Notification Integration
  @AC-1031.1
  Scenario: Toast notifications are shown for device connect/disconnect events
    Given the system is configured for REQ-1031
    When the feature condition is met
    Then toast notifications are shown for device connect/disconnect events

  @AC-1031.2
  Scenario: Notification categories are individually configurable
    Given the system is configured for REQ-1031
    When the feature condition is met
    Then notification categories are individually configurable

  @AC-1031.3
  Scenario: Notifications include actionable buttons for common responses
    Given the system is configured for REQ-1031
    When the feature condition is met
    Then notifications include actionable buttons for common responses

  @AC-1031.4
  Scenario: Notification permission is requested on first use
    Given the system is configured for REQ-1031
    When the feature condition is met
    Then notification permission is requested on first use
