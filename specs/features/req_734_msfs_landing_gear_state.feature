Feature: MSFS Landing Gear State
  As a flight simulation enthusiast
  I want the SimConnect adapter to track landing gear state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Gear up/down state tracked
    Given MSFS is running with a geared aircraft
    When the landing gear position changes
    Then the SimConnect adapter tracks the up/down state

  Scenario: State published on event bus
    Given the gear state changes
    When the adapter processes the change
    Then the new state is published on the event bus

  Scenario: In-transit state reported
    Given the landing gear is moving
    When the gear is between up and down
    Then an in-transit state is reported

  Scenario: State drives panel LEDs
    Given gear state is mapped to panel LEDs
    When the gear state changes
    Then the panel LED indicators update accordingly
