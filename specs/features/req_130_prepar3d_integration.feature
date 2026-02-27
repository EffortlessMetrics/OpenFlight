@REQ-130 @product
Feature: Prepar3D integration  @AC-130.1
  Scenario: SimConnect connection established with P3D v5
    Given Prepar3D v5 is running with the SimConnect server active
    When the adapter attempts to open a SimConnect session
    Then the connection SHALL succeed and the adapter state SHALL transition to connected  @AC-130.2
  Scenario: Aircraft state read via SimConnect variables
    Given an active SimConnect session to Prepar3D
    When the adapter requests PLANE ALTITUDE and AIRSPEED INDICATED variables
    Then the received values SHALL match the simulator state within one tick period  @AC-130.3
  Scenario: Control injection via SimConnect WriteClientData
    Given an active SimConnect session to Prepar3D
    When the adapter writes a pitch axis value via SimConnect WriteClientData
    Then the simulator SHALL reflect the updated control input on the next data cycle  @AC-130.4
  Scenario: Profile loads on aircraft change
    Given the adapter is in the active state
    When the simulated aircraft changes to a different model
    Then the profile matching the new aircraft SHALL be loaded within 500 ms  @AC-130.5
  Scenario: Reconnect after sim crash
    Given the adapter has an active SimConnect session
    When the Prepar3D process terminates unexpectedly
    Then the adapter SHALL detect the disconnection within 2 seconds
    And SHALL automatically attempt reconnection with exponential back-off  @AC-130.6
  Scenario: Adapter state machine transitions from idle to connected to active
    Given the adapter is in the idle state
    When a successful SimConnect connection is established
    Then the state SHALL progress idle → connected → active in order
    And no state SHALL be skipped
