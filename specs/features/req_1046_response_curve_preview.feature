@REQ-1046
Feature: Response Curve Preview
  @AC-1046.1
  Scenario: Live curve visualization shows input-to-output mapping graphically
    Given the system is configured for REQ-1046
    When the feature condition is met
    Then live curve visualization shows input-to-output mapping graphically

  @AC-1046.2
  Scenario: Preview updates in real-time as curve parameters are adjusted
    Given the system is configured for REQ-1046
    When the feature condition is met
    Then preview updates in real-time as curve parameters are adjusted

  @AC-1046.3
  Scenario: Current input position is highlighted on the curve display
    Given the system is configured for REQ-1046
    When the feature condition is met
    Then current input position is highlighted on the curve display

  @AC-1046.4
  Scenario: Preview supports all curve types including custom point curves
    Given the system is configured for REQ-1046
    When the feature condition is met
    Then preview supports all curve types including custom point curves
