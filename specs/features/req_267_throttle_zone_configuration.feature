@REQ-267 @product
Feature: Throttle axis supports configurable zones including cut, max, military power, and afterburner  @AC-267.1
  Scenario: Cut zone clips output to zero
    Given the throttle axis has a cut zone defined with a threshold
    When the raw axis value is below the cut threshold
    Then the zone processor SHALL output 0.0 regardless of the raw value  @AC-267.2
  Scenario: Max zone clips output to full power
    Given the throttle axis has a max zone defined with an upper threshold
    When the raw axis value is above the max threshold
    Then the zone processor SHALL clip the output to 1.0  @AC-267.3
  Scenario: Military power zone emits distinct event on entry
    Given the throttle has a military power zone configured between defined boundaries
    When the axis value crosses into the military power zone
    Then the zone processor SHALL emit a MilitaryPowerEntered event on the bus  @AC-267.4
  Scenario: Afterburner zone emits event on entry
    Given the throttle has an afterburner zone configured above the military power boundary
    When the axis value crosses into the afterburner zone
    Then the zone processor SHALL emit an AfterburnerEntered event on the bus  @AC-267.5
  Scenario: Zone disabled per profile suppresses processing
    Given the profile has the military power zone disabled
    When the axis value crosses the military power threshold
    Then no MilitaryPowerEntered event SHALL be emitted and normal scaling applies  @AC-267.6
  Scenario: Invalid zone boundaries rejected at profile load
    Given a profile defines a cut zone threshold that is greater than or equal to the max zone threshold
    When the profile is loaded
    Then the service SHALL reject the profile with a validation error describing the boundary conflict
