Feature: Axis State Machine Events
  As a flight simulation enthusiast
  I want the axis engine to emit state machine transition events
  So that profile rules and diagnostics can react to axis state changes

  Background:
    Given the OpenFlight service is running with axis processing active

  Scenario: State transitions are published as typed events on flight-bus
    When an axis transitions from one state to another
    Then a typed state transition event is published on flight-bus

  Scenario: Events include previous and new state and timestamp
    When a state transition event is emitted
    Then the event payload contains the previous state, new state, and a monotonic timestamp

  Scenario: State transition events can trigger profile rules
    Given a profile rule is configured to react to an axis state transition event
    When the axis transitions to the matching state
    Then the profile rule action is executed

  Scenario: Event rate is bounded to prevent bus flooding
    Given an axis oscillates rapidly between two states
    When events are emitted for each transition
    Then the event publication rate does not exceed the configured maximum events per second
