@REQ-1044
Feature: Axis Test Screen
  @AC-1044.1
  Scenario: Built-in axis test displays real-time axis positions for all connected devices
    Given the system is configured for REQ-1044
    When the feature condition is met
    Then built-in axis test displays real-time axis positions for all connected devices

  @AC-1044.2
  Scenario: Test screen shows raw and processed axis values side by side
    Given the system is configured for REQ-1044
    When the feature condition is met
    Then test screen shows raw and processed axis values side by side

  @AC-1044.3
  Scenario: Axis test supports visual indicators for deadzone and response curve
    Given the system is configured for REQ-1044
    When the feature condition is met
    Then axis test supports visual indicators for deadzone and response curve

  @AC-1044.4
  Scenario: Test screen is accessible via CLI and UI
    Given the system is configured for REQ-1044
    When the feature condition is met
    Then test screen is accessible via cli and ui
