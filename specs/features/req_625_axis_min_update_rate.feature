Feature: Axis Minimum Update Rate Enforcement
  As a flight simulation enthusiast
  I want the axis engine to enforce minimum update rate guarantees
  So that the simulation always receives a current or clearly stale axis value

  Background:
    Given the OpenFlight service is running

  Scenario: If no device input is received, last value is repeated at tick rate
    Given a device has previously sent axis input
    When no new input is received for the device
    Then the last known axis value is repeated at the engine tick rate

  Scenario: Repeated value includes a staleness flag
    Given the axis engine is repeating the last known value for a device
    When the axis output is inspected
    Then the output includes a staleness flag indicating the value is not fresh

  Scenario: Stale repeating triggers disconnection if no input for 5 seconds
    Given the axis engine has been repeating a stale value
    When 5 seconds elapse without any new input from the device
    Then the device is marked as disconnected

  Scenario: Minimum rate behavior is documented per device type
    When the device type documentation is inspected
    Then the minimum update rate behavior is described for each supported device type
