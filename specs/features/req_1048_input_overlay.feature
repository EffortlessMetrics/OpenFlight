@REQ-1048
Feature: Input Overlay
  @AC-1048.1
  Scenario: Transparent in-game overlay shows current input states
    Given the system is configured for REQ-1048
    When the feature condition is met
    Then transparent in-game overlay shows current input states

  @AC-1048.2
  Scenario: Overlay position and opacity are configurable
    Given the system is configured for REQ-1048
    When the feature condition is met
    Then overlay position and opacity are configurable

  @AC-1048.3
  Scenario: Overlay can be toggled via configurable hotkey
    Given the system is configured for REQ-1048
    When the feature condition is met
    Then overlay can be toggled via configurable hotkey

  @AC-1048.4
  Scenario: Overlay rendering does not impact sim frame rate measurably
    Given the system is configured for REQ-1048
    When the feature condition is met
    Then overlay rendering does not impact sim frame rate measurably
