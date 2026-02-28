@REQ-709
Feature: Axis Gate Boundary Definition
  @AC-709.1
  Scenario: Gate boundaries divide the axis range into discrete zones
    Given the system is configured for REQ-709
    When the feature condition is met
    Then gate boundaries divide the axis range into discrete zones

  @AC-709.2
  Scenario: Gate transitions are reported as discrete events
    Given the system is configured for REQ-709
    When the feature condition is met
    Then gate transitions are reported as discrete events

  @AC-709.3
  Scenario: Gate boundary positions are configurable in profile
    Given the system is configured for REQ-709
    When the feature condition is met
    Then gate boundary positions are configurable in profile

  @AC-709.4
  Scenario: Gate zones can trigger profile-defined actions
    Given the system is configured for REQ-709
    When the feature condition is met
    Then gate zones can trigger profile-defined actions
