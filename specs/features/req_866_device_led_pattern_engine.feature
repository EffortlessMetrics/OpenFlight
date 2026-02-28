Feature: Device LED Pattern Engine
  As a flight simulation enthusiast
  I want device led pattern engine
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Programmable LED sequences can be defined with timing and color
    Given the system is configured for device led pattern engine
    When the feature is exercised
    Then programmable LED sequences can be defined with timing and color

  Scenario: LED patterns can be triggered by sim events or state changes
    Given the system is configured for device led pattern engine
    When the feature is exercised
    Then lED patterns can be triggered by sim events or state changes

  Scenario: Pattern engine supports blending multiple concurrent patterns
    Given the system is configured for device led pattern engine
    When the feature is exercised
    Then pattern engine supports blending multiple concurrent patterns

  Scenario: Invalid pattern definitions are rejected with descriptive errors
    Given the system is configured for device led pattern engine
    When the feature is exercised
    Then invalid pattern definitions are rejected with descriptive errors
