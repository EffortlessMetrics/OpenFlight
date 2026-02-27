@REQ-279 @product
Feature: Throttle zone implementation with cut zone saturation, military power and afterburner zone events  @AC-279.1
  Scenario: Cut zone threshold maps raw throttle to 0.0
    Given a throttle axis configured with a cut zone threshold at 0.05
    When the raw throttle input is at or below the cut zone threshold
    Then the processed axis output SHALL be clamped to 0.0  @AC-279.2
  Scenario: Max zone saturates output to 1.0 above threshold
    Given a throttle axis configured with a max zone threshold at 0.95
    When the raw throttle input is at or above the max zone threshold
    Then the processed axis output SHALL be saturated to 1.0  @AC-279.3
  Scenario: Military power zone emits ZoneEntered event
    Given a throttle axis with a military power zone defined between 0.85 and 0.95
    When the throttle value crosses into the military power zone boundary
    Then a ZoneEntered event SHALL be emitted on the event bus  @AC-279.4
  Scenario: Afterburner zone emits ZoneEntered event with zone name
    Given a throttle axis with an afterburner zone defined above 0.95
    When the throttle value crosses into the afterburner zone
    Then a ZoneEntered event SHALL be emitted containing the zone name "afterburner"  @AC-279.5
  Scenario: Zone configurations are validated on profile load
    Given a profile containing a throttle zone configuration with overlapping zone boundaries
    When the profile is loaded by the service
    Then the service SHALL reject the profile with a zone boundary validation error  @AC-279.6
  Scenario: Zones can be independently enabled or disabled
    Given a throttle axis profile with multiple zones defined
    When one zone is disabled in the configuration
    Then that zone SHALL not emit events or affect output while the other zones remain active
