@REQ-1006
Feature: Voice Command Integration
  @AC-1006.1
  Scenario: Voice commands can be mapped to input actions in profile
    Given the system is configured for REQ-1006
    When the feature condition is met
    Then voice commands can be mapped to input actions in profile

  @AC-1006.2
  Scenario: Voice recognition engine is configurable between platform options
    Given the system is configured for REQ-1006
    When the feature condition is met
    Then voice recognition engine is configurable between platform options

  @AC-1006.3
  Scenario: WHEN a recognized voice command is detected THEN the mapped action SHALL execute
    Given the system is configured for REQ-1006
    When the feature condition is met
    Then when a recognized voice command is detected then the mapped action shall execute

  @AC-1006.4
  Scenario: Voice command confidence threshold is configurable to reduce false triggers
    Given the system is configured for REQ-1006
    When the feature condition is met
    Then voice command confidence threshold is configurable to reduce false triggers
