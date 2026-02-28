@REQ-1037
Feature: Windows Game Bar
  @AC-1037.1
  Scenario: Game Bar widget displays active control status during gameplay
    Given the system is configured for REQ-1037
    When the feature condition is met
    Then game bar widget displays active control status during gameplay

  @AC-1037.2
  Scenario: Widget shows connected devices and active profile name
    Given the system is configured for REQ-1037
    When the feature condition is met
    Then widget shows connected devices and active profile name

  @AC-1037.3
  Scenario: Quick actions in the widget allow profile switching
    Given the system is configured for REQ-1037
    When the feature condition is met
    Then quick actions in the widget allow profile switching

  @AC-1037.4
  Scenario: Widget updates in real-time without impacting game performance
    Given the system is configured for REQ-1037
    When the feature condition is met
    Then widget updates in real-time without impacting game performance
