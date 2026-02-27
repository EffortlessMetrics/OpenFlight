Feature: SimConnect Aircraft Loaded Event
  As a flight simulation enthusiast
  I want the SimConnect adapter to handle aircraft-loaded events
  So that profiles switch automatically when I change aircraft in MSFS

  Background:
    Given the OpenFlight service is running
    And the SimConnect adapter is connected to MSFS

  Scenario: AIRCRAFT_LOADED event triggers aircraft detection update
    When MSFS fires the AIRCRAFT_LOADED SimConnect event with title "Cessna 172 Skyhawk"
    Then the aircraft detector updates the current aircraft to "Cessna 172 Skyhawk"

  Scenario: New aircraft title and type are reported via flight-bus
    When MSFS fires AIRCRAFT_LOADED with title "Airbus A320neo"
    Then the flight-bus receives an AircraftChanged event containing the title and detected type

  Scenario: Profile auto-select runs after aircraft load event
    Given a profile rule matches aircraft title "Cessna 172 Skyhawk"
    When the AIRCRAFT_LOADED event fires for "Cessna 172 Skyhawk"
    Then the matching profile is automatically activated

  Scenario: Aircraft load events are counted in adapter metrics
    Given the adapter metrics counter "aircraft_loaded_events_total" starts at 0
    When three AIRCRAFT_LOADED events are received
    Then the counter "aircraft_loaded_events_total" equals 3
