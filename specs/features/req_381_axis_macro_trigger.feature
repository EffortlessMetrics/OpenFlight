@REQ-381 @product
Feature: Axis Macro Trigger on Input Value Threshold  @AC-381.1
  Scenario: Macro triggers when axis value exceeds a configured threshold
    Given an axis macro configured with a trigger threshold of 0.8
    When the axis value crosses above 0.8
    Then the macro SHALL be triggered  @AC-381.2
  Scenario: Rising edge, falling edge, and both directions are configurable
    Given axis macros configured for rising edge, falling edge, and both edge modes
    When axis values cross thresholds in both directions
    Then each macro SHALL trigger only on its configured edge direction  @AC-381.3
  Scenario: Threshold hysteresis prevents re-triggering on noise
    Given an axis macro with hysteresis configured at 0.05
    When the axis value oscillates within the hysteresis band after crossing
    Then the macro SHALL NOT fire again until the axis leaves the hysteresis band  @AC-381.4
  Scenario: Macro trigger emits a MacroTriggerEvent on the bus
    Given an axis macro is configured and the axis crosses its threshold
    When the trigger fires
    Then a MacroTriggerEvent SHALL be emitted on the event bus  @AC-381.5
  Scenario: Multiple macros can be bound to the same axis with different thresholds
    Given two macros bound to the same axis with thresholds 0.5 and 0.9
    When the axis value reaches 0.5 and later 0.9
    Then each macro SHALL trigger independently at its respective threshold  @AC-381.6
  Scenario: Property test confirms hysteresis prevents consecutive re-triggering
    Given a property test with arbitrary axis values and a hysteresis configuration
    When axis values are applied sequentially
    Then the same trigger SHALL NOT fire on two consecutive samples without leaving the hysteresis band
