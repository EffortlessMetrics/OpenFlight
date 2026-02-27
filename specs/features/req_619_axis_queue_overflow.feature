Feature: Axis Engine Input Queue Overflow Handling
  As a flight simulation enthusiast
  I want the axis engine to handle input queue overflow gracefully
  So that queue saturation does not cause crashes or data corruption

  Background:
    Given the OpenFlight service is running

  Scenario: Queue overflow drops oldest samples rather than newest
    Given the axis input queue is configured with a fixed capacity
    When more samples arrive than the queue can hold
    Then the oldest samples are dropped to make room for the newest

  Scenario: Overflow events are counted in engine diagnostics
    Given the axis input queue has overflowed
    When the engine diagnostics are queried
    Then the overflow event count reflects the number of dropped samples

  Scenario: Overflow does not cause panic or undefined behavior
    When the axis input queue is saturated with rapid input
    Then the engine continues processing without panicking or corrupting state

  Scenario: Queue capacity is configurable at engine initialization
    When the axis engine is initialized with a custom queue capacity
    Then the engine uses the specified capacity for its input queue
