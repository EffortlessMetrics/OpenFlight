@REQ-554 @product
Feature: Axis Boost Mode — Axis engine should support temporary sensitivity boost

  @AC-554.1
  Scenario: Boost multiplies axis sensitivity by configurable factor
    Given an axis with a boost factor of 2.0 configured
    When boost mode is active
    Then the axis sensitivity SHALL be multiplied by 2.0

  @AC-554.2
  Scenario: Boost is activated by a mapped button
    Given a button mapped to activate boost mode
    When that button is pressed
    Then boost mode SHALL become active immediately

  @AC-554.3
  Scenario: Boost deactivates when button is released
    Given boost mode is currently active
    When the mapped boost button is released
    Then boost mode SHALL deactivate and sensitivity SHALL return to baseline

  @AC-554.4
  Scenario: Boost does not bypass safety clamping
    Given boost mode is active with a factor that would exceed the output range
    When the boosted axis value is computed
    Then the output SHALL still be clamped to the configured safe output range
