Feature: Bus Event Filtering
  As a flight simulation enthusiast
  I want bus subscribers to support event type filtering
  So that components only receive the events they need

  Background:
    Given the OpenFlight service is running
    And the flight-bus is active

  Scenario: Subscriber can declare interest in specific event types
    When a subscriber registers with an event filter for "AxisUpdate" events only
    Then the subscription is recorded with the "AxisUpdate" filter

  Scenario: Filtered events are not delivered to uninterested subscribers
    Given a subscriber is registered with a filter for "AxisUpdate" events only
    When a "ProfileChanged" event is published on the bus
    Then the subscriber does not receive the "ProfileChanged" event

  Scenario: Filter configuration applies at subscription time
    When a subscriber registers without a filter
    Then the subscriber receives all event types published on the bus

  Scenario: Filter changes take effect within one bus tick
    Given a subscriber is registered with a filter for "AxisUpdate" events
    When the filter is updated to include "DeviceConnected" events
    Then within one bus tick the subscriber receives both event types
