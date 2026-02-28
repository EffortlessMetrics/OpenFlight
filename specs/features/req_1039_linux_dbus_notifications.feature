@REQ-1039
Feature: Linux DBus Notifications
  @AC-1039.1
  Scenario: Notifications are sent via DBus notification interface on Linux
    Given the system is configured for REQ-1039
    When the feature condition is met
    Then notifications are sent via dbus notification interface on linux

  @AC-1039.2
  Scenario: Notification urgency level matches event severity
    Given the system is configured for REQ-1039
    When the feature condition is met
    Then notification urgency level matches event severity

  @AC-1039.3
  Scenario: Notification actions are supported for interactive responses
    Given the system is configured for REQ-1039
    When the feature condition is met
    Then notification actions are supported for interactive responses

  @AC-1039.4
  Scenario: DBus notification support is detected and enabled automatically
    Given the system is configured for REQ-1039
    When the feature condition is met
    Then dbus notification support is detected and enabled automatically
