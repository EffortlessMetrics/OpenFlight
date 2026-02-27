@REQ-1034
Feature: Taskbar Progress
  @AC-1034.1
  Scenario: Long-running operations show progress in Windows taskbar
    Given the system is configured for REQ-1034
    When the feature condition is met
    Then long-running operations show progress in windows taskbar

  @AC-1034.2
  Scenario: Progress indicator uses appropriate state: normal, paused, or error
    Given the system is configured for REQ-1034
    When the feature condition is met
    Then progress indicator uses appropriate state: normal, paused, or error

  @AC-1034.3
  Scenario: Progress is shown for firmware updates, calibration, and profile import
    Given the system is configured for REQ-1034
    When the feature condition is met
    Then progress is shown for firmware updates, calibration, and profile import

  @AC-1034.4
  Scenario: Taskbar progress clears automatically on operation completion
    Given the system is configured for REQ-1034
    When the feature condition is met
    Then taskbar progress clears automatically on operation completion
