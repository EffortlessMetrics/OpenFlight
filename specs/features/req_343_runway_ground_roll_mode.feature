@REQ-343 @product
Feature: Runway / Ground Roll Mode  @AC-343.1
  Scenario: Service detects ground roll phase
    Given the aircraft has gear down and ground speed below the takeoff threshold
    When the service evaluates the flight phase
    Then the service SHALL enter ground roll mode  @AC-343.2
  Scenario: Rudder axis is scaled during ground roll
    Given the service is in ground roll mode
    When a rudder axis input is received
    Then the service SHALL apply ground roll scaling to the rudder axis output  @AC-343.3
  Scenario: Takeoff speed threshold is configurable per aircraft profile
    Given an aircraft profile specifies a takeoff speed threshold of 80 knots
    When the aircraft ground speed is below 80 knots with gear down
    Then the service SHALL use that aircraft-specific threshold to determine ground roll mode  @AC-343.4
  Scenario: Ground roll mode is reflected in active profile slot
    Given the service enters ground roll mode
    When the active profile slot is queried
    Then the active profile slot SHALL reflect the ground roll mode state  @AC-343.5
  Scenario: Mode changes are emitted on the bus
    Given the service transitions between ground roll mode and normal mode
    When the mode change occurs
    Then a mode change event SHALL be emitted on the event bus  @AC-343.6
  Scenario: Ground roll scaling reverts to default when airborne
    Given the service is in ground roll mode
    When the aircraft becomes airborne (weight off wheels)
    Then the rudder axis scaling SHALL revert to the default profile value
