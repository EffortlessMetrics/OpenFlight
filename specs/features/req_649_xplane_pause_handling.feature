Feature: X-Plane Simulator Paused Handling
  As a flight simulation enthusiast
  I want the X-Plane adapter to handle simulator paused state
  So that FFB and profile rules respond correctly when the sim is paused

  Background:
    Given the OpenFlight service is running and connected to X-Plane

  Scenario: Pause state is detected from X-Plane dataref sim/time/paused
    When the X-Plane simulator is paused
    Then the adapter reads dataref "sim/time/paused" as 1

  Scenario: Paused state is published on flight-bus
    Given the X-Plane simulator transitions from running to paused
    When the pause is detected by the adapter
    Then a "SimulatorPaused" event is published on the flight-bus

  Scenario: FFB effects are ramped to idle when sim is paused
    Given the simulator is paused and FFB effects are active
    When the "SimulatorPaused" event is received by the FFB engine
    Then all FFB effects are ramped down to idle over 200ms

  Scenario: Profile rules can react to pause state changes
    Given a profile rule is configured to trigger on "SimulatorPaused" event
    When the simulator is paused
    Then the configured rule action is executed
