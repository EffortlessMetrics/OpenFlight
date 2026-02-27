@REQ-141 @product
Feature: X-Plane 12 specific features  @AC-141.1
  Scenario: Autopilot engaged state decoded from dataref
    Given an X-Plane 12 UDP connection is active
    When the autopilot-engaged dataref reports value 1
    Then the adapter SHALL report autopilot state as engaged  @AC-141.2
  Scenario: Flaps fully deployed decoded from dataref
    Given an X-Plane 12 UDP connection is active
    When the flaps ratio dataref reports value 1.0
    Then the adapter SHALL report flaps as fully deployed  @AC-141.3
  Scenario: Landing gear down state decoded from dataref
    Given an X-Plane 12 UDP connection is active
    When the gear deployment dataref reports value 1.0
    Then the adapter SHALL report landing gear as down  @AC-141.4
  Scenario: Multi-engine N1 values are distinct per engine
    Given an X-Plane 12 multi-engine aircraft with four engines
    When each engine reports a distinct N1 dataref value
    Then the adapter SHALL surface four distinct N1 readings  @AC-141.5
  Scenario: AoA alpha dataref parsed correctly
    Given an X-Plane 12 UDP connection is active
    When the alpha dataref reports 8.5 degrees
    Then the adapter SHALL report angle of attack as 8.5 degrees  @AC-141.6
  Scenario: FMC connection via UDP DataRef
    Given an X-Plane 12 instance with the DataRef UDP plugin active
    When the FMC route dataref is subscribed
    Then the adapter SHALL receive FMC route data over the UDP DataRef channel  @AC-141.7
  Scenario: Weather turbulence intensity dataref parsed
    Given an X-Plane 12 UDP connection is active
    When the turbulence intensity dataref reports 0.65
    Then the adapter SHALL report turbulence intensity as 0.65  @AC-141.8
  Scenario: Vertical speed from dataref in feet per minute
    Given an X-Plane 12 UDP connection is active
    When the vertical speed dataref reports -500 fpm
    Then the adapter SHALL report vertical speed as -500 fpm
