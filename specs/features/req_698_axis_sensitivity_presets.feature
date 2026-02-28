@REQ-698
Feature: Axis Sensitivity Presets
  @AC-698.1
  Scenario: Predefined sensitivity presets are available per axis type
    Given the system is configured for REQ-698
    When the feature condition is met
    Then predefined sensitivity presets are available per axis type

  @AC-698.2
  Scenario: Presets cover low, medium, high, and custom sensitivity
    Given the system is configured for REQ-698
    When the feature condition is met
    Then presets cover low, medium, high, and custom sensitivity

  @AC-698.3
  Scenario: Selecting a preset configures curve and deadzone together
    Given the system is configured for REQ-698
    When the feature condition is met
    Then selecting a preset configures curve and deadzone together

  @AC-698.4
  Scenario: Custom preset can be saved from current axis settings
    Given the system is configured for REQ-698
    When the feature condition is met
    Then custom preset can be saved from current axis settings
