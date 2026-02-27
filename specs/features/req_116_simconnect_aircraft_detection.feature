@REQ-116 @product
Feature: SimConnect aircraft type detection and profile cascade

  @AC-116.1
  Scenario: Aircraft title containing "Boeing 737" maps to Boeing737 aircraft type
    Given a SimConnect adapter
    When an aircraft title of "Boeing 737-800" is received
    Then the detected aircraft type SHALL be Boeing737

  @AC-116.2
  Scenario: Aircraft title containing "Airbus A320" maps to AirbusA320 aircraft type
    Given a SimConnect adapter
    When an aircraft title of "Airbus A320neo" is received
    Then the detected aircraft type SHALL be AirbusA320

  @AC-116.3
  Scenario: Unrecognised aircraft title returns generic profile type
    Given a SimConnect adapter
    When an aircraft title of "Some Unknown Experimental 2024" is received
    Then the detected aircraft type SHALL be Generic
    And the active profile SHALL be the generic fallback profile

  @AC-116.4
  Scenario: Aircraft change event is published on the bus
    Given a SimConnect adapter connected to the flight bus
    When the active aircraft changes from one title to another
    Then an AircraftChangedEvent SHALL be published on the bus
    And the event SHALL contain the new aircraft title

  @AC-116.5
  Scenario: Profile cascade fires when aircraft type changes
    Given a SimConnect adapter with a profile configured for Boeing737
    When an aircraft title matching Boeing737 is received
    Then the profile cascade SHALL be triggered
    And the Boeing737-specific profile overlay SHALL be applied on top of the global profile

  @AC-116.5
  Scenario: Profile cascade reverts to global profile for generic aircraft
    Given a SimConnect adapter that previously had a Boeing737 profile active
    When an unrecognised aircraft title is received
    Then the profile cascade SHALL fall back to the global profile
    And the Boeing737 overlay SHALL be removed
