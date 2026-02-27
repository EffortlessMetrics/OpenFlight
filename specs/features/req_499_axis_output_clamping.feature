@REQ-499 @product
Feature: Axis Output Clamping — Final Output Clamped to Safe Range  @AC-499.1
  Scenario: Output clamp range is configurable per axis
    Given an axis is configured with a custom clamp range of -0.9 to 0.9
    When the axis pipeline produces an output value of 1.0
    Then the output SHALL be clamped to 0.9  @AC-499.2
  Scenario: Clamp events are counted in axis diagnostics
    Given an axis with default clamp range -1.0 to 1.0
    When the pipeline output exceeds 1.0 three times in a session
    Then the axis diagnostics SHALL report a clamp event count of 3  @AC-499.3
  Scenario: Clamp is applied as the final processing stage
    Given an axis with curve and deadzone configured
    When the axis value is processed through the full pipeline
    Then the clamp stage SHALL execute after all other stages and before output  @AC-499.4
  Scenario: Asymmetric clamp supports different positive and negative limits
    Given an axis configured with asymmetric clamp min -0.8 and max 1.0
    When the axis output is -1.0
    Then the clamped output SHALL be -0.8
