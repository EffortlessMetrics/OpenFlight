@REQ-449 @product
Feature: FFB Envelope Safety Clamp — Clamp FFB Outputs to a Safe Force Envelope

  @AC-449.1
  Scenario: FFB output force is clamped to configured maximum force percentage
    Given the FFB safety envelope is configured with a maximum of 80%
    When the composed effect requests 100% force
    Then the output SHALL be clamped to 80% of the device's maximum force

  @AC-449.2
  Scenario: Force clamp is applied after all effect composition
    Given multiple FFB effects are active and composited
    When the composed total exceeds the envelope maximum
    Then the clamp SHALL be applied to the final composed value, not to individual effects

  @AC-449.3
  Scenario: Clamp activations are counted and logged
    Given the FFB engine is running
    When the safety clamp activates
    Then the activation SHALL increment a clamp_activations counter and emit a log entry at warn level

  @AC-449.4
  Scenario: Emergency stop clamps to zero and logs a warning immediately
    Given an emergency stop is triggered
    When the FFB engine processes the next update cycle
    Then all force output SHALL be clamped to zero and a warning SHALL be logged immediately
