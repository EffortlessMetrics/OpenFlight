@REQ-1032
Feature: System Tray Icon
  @AC-1032.1
  Scenario: Service status is shown via system tray icon
    Given the system is configured for REQ-1032
    When the feature condition is met
    Then service status is shown via system tray icon

  @AC-1032.2
  Scenario: Tray icon context menu provides quick access to common actions
    Given the system is configured for REQ-1032
    When the feature condition is met
    Then tray icon context menu provides quick access to common actions

  @AC-1032.3
  Scenario: Icon appearance changes to reflect service health status
    Given the system is configured for REQ-1032
    When the feature condition is met
    Then icon appearance changes to reflect service health status

  @AC-1032.4
  Scenario: Tray icon tooltip shows active profile and connected device count
    Given the system is configured for REQ-1032
    When the feature condition is met
    Then tray icon tooltip shows active profile and connected device count
