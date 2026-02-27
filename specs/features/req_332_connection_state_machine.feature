@REQ-332 @product
Feature: Connection State Machine  @AC-332.1
  Scenario: Adapter connections follow the defined state machine
    Given an adapter in the Disconnected state
    When a connection is initiated
    Then the adapter SHALL transition through Disconnected → Connecting → Connected → Active → Disconnected in valid sequences  @AC-332.2
  Scenario: State transitions are logged with timestamps
    Given an adapter undergoing a state transition
    When the transition occurs
    Then a log entry SHALL be emitted containing the previous state, new state, and a UTC timestamp  @AC-332.3
  Scenario: Invalid state transitions produce a warning and no-op
    Given an adapter in the Active state
    When a Connect event is received (invalid for Active)
    Then the service SHALL log a WARN and leave the adapter state unchanged  @AC-332.4
  Scenario: State machine is unit-tested with all valid transitions
    Given the connection state machine implementation
    When the unit test suite is executed
    Then tests SHALL cover every valid state transition in the machine  @AC-332.5
  Scenario: Pending state prevents duplicate connection attempts
    Given an adapter already in the Connecting state
    When another connection attempt is requested
    Then the service SHALL ignore the duplicate attempt and remain in Connecting  @AC-332.6
  Scenario: State changes trigger events on the internal bus
    Given an adapter whose state changes
    When the transition completes
    Then the service SHALL publish a ConnectionStateChanged event on the internal flight-bus
