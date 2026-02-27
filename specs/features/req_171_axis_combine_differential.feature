@REQ-171 @product
Feature: Axis combine differential implementation

  @AC-171.1
  Scenario: Left pedal full forward maps to full positive rudder
    Given a differential combine mapping with left pedal at 1.0 and right pedal at 0.0
    When the combine_differential operation is applied
    Then the rudder axis output SHALL be +1.0

  @AC-171.2
  Scenario: Right pedal full forward maps to full negative rudder
    Given a differential combine mapping with left pedal at 0.0 and right pedal at 1.0
    When the combine_differential operation is applied
    Then the rudder axis output SHALL be -1.0

  @AC-171.3
  Scenario: Equal pedal positions produce zero rudder
    Given a differential combine mapping with both pedals at the same position
    When the combine_differential operation is applied
    Then the rudder axis output SHALL be 0.0

  @AC-171.4
  Scenario: Both pedals at maximum are clamped to valid range
    Given a differential combine mapping with both pedals at their physical maximum
    When the combine_differential operation is applied
    Then the combined axis output SHALL be clamped to the range [-1.0, 1.0]

  @AC-171.5
  Scenario: combine_average of equal inputs equals the input value
    Given two axis inputs with the same value V
    When the combine_average operation is applied
    Then the output SHALL equal V

  @AC-171.6
  Scenario: split_bipolar +1.0 returns (1.0, 0.0)
    Given a bipolar axis value of +1.0
    When the split_bipolar operation is applied
    Then the positive channel output SHALL be 1.0 and the negative channel output SHALL be 0.0

  @AC-171.7
  Scenario: split_bipolar -1.0 returns (0.0, 1.0)
    Given a bipolar axis value of -1.0
    When the split_bipolar operation is applied
    Then the positive channel output SHALL be 0.0 and the negative channel output SHALL be 1.0
