@REQ-1047
Feature: Deadzone Preview
  @AC-1047.1
  Scenario: Visual deadzone display shows active and inactive regions
    Given the system is configured for REQ-1047
    When the feature condition is met
    Then visual deadzone display shows active and inactive regions

  @AC-1047.2
  Scenario: Preview updates in real-time as deadzone parameters change
    Given the system is configured for REQ-1047
    When the feature condition is met
    Then preview updates in real-time as deadzone parameters change

  @AC-1047.3
  Scenario: Current input position relative to deadzone is clearly indicated
    Given the system is configured for REQ-1047
    When the feature condition is met
    Then current input position relative to deadzone is clearly indicated

  @AC-1047.4
  Scenario: Preview supports both inner and outer deadzone visualization
    Given the system is configured for REQ-1047
    When the feature condition is met
    Then preview supports both inner and outer deadzone visualization
