@REQ-439 @product
Feature: FFB Gain Control — Global Gain Adjustment for the FFB Engine

  @AC-439.1
  Scenario: Global FFB gain is configurable from 0.0 to 1.0
    Given a running FFB engine
    When the global gain is set to 0.75
    Then the engine SHALL accept values in the range [0.0, 1.0] without error

  @AC-439.2
  Scenario: Gain change takes effect within one FFB update cycle
    Given the FFB engine is running at its update rate
    When the global gain is changed
    Then the new gain SHALL be applied to all effects within one update cycle

  @AC-439.3
  Scenario: Zero gain disables all effects immediately
    Given active FFB effects are playing
    When the global gain is set to 0.0
    Then all force output SHALL be silenced in the same update cycle

  @AC-439.4
  Scenario: Gain is persisted in user profile and restored on service restart
    Given the global gain has been set to 0.6 and saved
    When the service is restarted
    Then the FFB engine SHALL restore global gain to 0.6 from the user profile
