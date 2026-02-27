@REQ-526 @product
Feature: Axis Input Combination Blend — Blending Two Physical Axes into One Virtual Axis  @AC-526.1
  Scenario: Blend combines two axes using a configurable factor
    Given two physical axes A and B and a blend factor of 0.5
    When both axes report a value
    Then the virtual axis output SHALL equal (A * 0.5) + (B * 0.5)  @AC-526.2
  Scenario: Factor 0.0 produces only the first axis value
    Given a blend configuration with factor 0.0
    When axis A reports 0.8 and axis B reports 0.2
    Then the virtual axis output SHALL equal 0.8  @AC-526.3
  Scenario: Blend factor can be driven by a third physical axis
    Given a blend configuration where the factor is bound to axis C
    When axis C reports 0.75
    Then the blend factor SHALL be updated to 0.75 and applied on the next tick  @AC-526.4
  Scenario: Blend output is clamped to the valid output range
    Given a blend of two saturated axes each reporting 1.0 with factor 0.5
    When the blend is computed
    Then the output SHALL be clamped to the maximum valid normalised value of 1.0
