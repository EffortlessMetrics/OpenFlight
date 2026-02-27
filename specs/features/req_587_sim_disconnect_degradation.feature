Feature: Sim Disconnect Graceful Degradation
  As a flight simulation enthusiast
  I want the service to degrade gracefully when the simulator disconnects
  So that my hardware continues to function and reconnects automatically

  Background:
    Given the OpenFlight service is running
    And the simulator adapter is connected

  Scenario: Sim disconnect is detected within 2 seconds
    When the simulator process exits unexpectedly
    Then the adapter detects the disconnect within 2 seconds
    And a "SimDisconnected" event is published on the bus

  Scenario: Axis engine continues running after sim disconnect
    Given a sim disconnect event has been detected
    When the axis engine processes the next tick
    Then the axis engine continues running normally without the simulator

  Scenario: FFB effects are ramped to zero on sim disconnect
    Given active force feedback effects are running
    When a sim disconnect event is detected
    Then all FFB force values are ramped to zero within 500 milliseconds

  Scenario: Service auto-reconnects when sim becomes available
    Given the service is in a sim-disconnected state
    When the simulator becomes available again
    Then the adapter reconnects automatically and publishes a "SimConnected" event
