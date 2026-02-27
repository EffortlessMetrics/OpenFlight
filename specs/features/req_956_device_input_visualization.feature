Feature: Device Input Visualization
  As a flight simulation enthusiast
  I want device input visualization
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Real-time input display shows current axis positions for all connected devices
    Given the system is configured for device input visualization
    When the feature is exercised
    Then real-time input display shows current axis positions for all connected devices

  Scenario: Visualization updates at display refresh rate without impacting RT processing
    Given the system is configured for device input visualization
    When the feature is exercised
    Then visualization updates at display refresh rate without impacting RT processing

  Scenario: Button press states are displayed with visual indicators
    Given the system is configured for device input visualization
    When the feature is exercised
    Then button press states are displayed with visual indicators

  Scenario: Input visualization supports multiple simultaneous device views
    Given the system is configured for device input visualization
    When the feature is exercised
    Then input visualization supports multiple simultaneous device views