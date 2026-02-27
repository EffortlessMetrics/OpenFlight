@REQ-390 @product
Feature: Adaptive EMA Smoothing Based on Axis Velocity  @AC-390.1
  Scenario: Adaptive EMA uses lower alpha when axis velocity is low
    Given an axis configured with adaptive EMA and a velocity_threshold
    When axis input changes slowly below velocity_threshold
    Then the EMA SHALL apply alpha_slow for stronger smoothing  @AC-390.2
  Scenario: Adaptive EMA uses higher alpha when axis velocity is high
    Given an axis configured with adaptive EMA and a velocity_threshold
    When axis input changes quickly above velocity_threshold
    Then the EMA SHALL apply alpha_fast for lighter smoothing  @AC-390.3
  Scenario: Alpha transition between slow and fast is smooth without discontinuities
    Given an axis velocity crossing the velocity_threshold boundary
    When alpha blends between alpha_slow and alpha_fast
    Then the output SHALL transition continuously with no sudden discontinuities  @AC-390.4
  Scenario: Config specifies alpha_slow, alpha_fast, and velocity_threshold
    Given a profile with adaptive EMA configured for an axis
    When the configuration is validated
    Then alpha_slow, alpha_fast, and velocity_threshold SHALL all be required fields  @AC-390.5
  Scenario: Property test confirms output stays within [-1, 1] for valid inputs
    Given a property test with arbitrary inputs in [-1, 1] and a valid adaptive EMA config
    When adaptive EMA is applied to all inputs
    Then all output values SHALL remain within [-1.0, 1.0]  @AC-390.6
  Scenario: No heap allocation and state fits in a fixed-size struct
    Given the adaptive EMA filter struct initialized at service startup
    When the filter processes axis values in the RT loop
    Then no heap allocation SHALL occur and the struct size SHALL be known at compile time
