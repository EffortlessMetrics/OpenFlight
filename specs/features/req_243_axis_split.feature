@REQ-243 @product
Feature: Bipolar axis split to dual unipolar channels for differential controls  @AC-243.1
  Scenario: Split bipolar axis into positive and negative channels
    Given a bipolar axis input X in the range [-1.0, 1.0]
    When the axis split transform is applied
    Then the positive channel SHALL equal max(X, 0) and the negative channel SHALL equal max(-X, 0)  @AC-243.2
  Scenario: Split outputs clamped to unit range
    Given a bipolar axis with an out-of-range raw value
    When the split transform clamps the outputs
    Then both the positive and negative channel values SHALL be clamped to [0.0, 1.0]  @AC-243.3
  Scenario: Differential braking maps stick deflection to independent brake channels
    Given a yaw axis configured for differential braking in the profile
    When the stick is deflected fully left
    Then the left brake channel SHALL output 1.0 and the right brake channel SHALL output 0.0  @AC-243.4
  Scenario: Combined and split modes are configurable per axis in profile
    Given an axis profile entry with mode set to split
    When the profile is loaded and applied to the axis engine
    Then the axis SHALL operate in split mode and another axis with mode combined SHALL operate in combined mode  @AC-243.5
  Scenario: Split operation functions correctly in combination with detents
    Given a bipolar axis profile with both a centre detent and split mode enabled
    When the axis value traverses the detent zone
    Then the split channels SHALL reflect the snapped output value and the detent SHALL engage at the expected threshold  @AC-243.6
  Scenario: Split axis outputs appear on event bus as two named virtual axes
    Given a bipolar axis named rudder configured in split mode
    When the axis engine publishes its outputs to the event bus
    Then two virtual axis events named rudder_positive and rudder_negative SHALL be present on the bus
