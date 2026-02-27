@REQ-166 @product
Feature: Axis combining and splitting

  @AC-166.1
  Scenario: Two physical pedals combined into differential rudder
    Given a profile mapping two physical pedal axes to a combined differential rudder
    When the left and right pedal positions are read
    Then the combined rudder axis SHALL output the differential value of the two pedals

  @AC-166.2
  Scenario: Single throttle axis split into twin engine levers
    Given a profile splitting one physical throttle axis into two virtual engine levers
    When the physical throttle axis is moved
    Then both virtual engine lever axes SHALL track the physical axis position independently

  @AC-166.3
  Scenario: Combined axis uses correct differential math
    Given two pedal axes with positions L and R
    When the combine operation is applied
    Then the combined axis output SHALL equal (L - R) / 2

  @AC-166.4
  Scenario: Combined axis deadzone centered at hardware neutral
    Given a combined differential axis with a deadzone configured
    When both pedals are at their hardware neutral positions
    Then the combined axis output SHALL be zero within the deadzone

  @AC-166.5
  Scenario: Profile defines combine and split without code change
    Given a profile YAML that defines axis combine and split operations
    When the profile is loaded
    Then the axis combine and split mappings SHALL be applied without requiring a code change or restart

  @AC-166.6
  Scenario: Combined axis output range is [-1.0, 1.0]
    Given a combined differential axis
    When either pedal is at its physical extreme
    Then the combined axis output SHALL be clamped to the range [-1.0, 1.0]
