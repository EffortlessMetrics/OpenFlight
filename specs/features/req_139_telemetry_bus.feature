@REQ-139 @infra
Feature: Streaming telemetry bus  @AC-139.1
  Scenario: Bus snapshot has monotonic timestamp
    Given a telemetry bus receiving successive publishes
    When two consecutive snapshots are captured
    Then the timestamp of the second snapshot SHALL be greater than or equal to the timestamp of the first  @AC-139.2
  Scenario: Multiple publishers do not corrupt shared state
    Given a telemetry bus with three concurrent publishers each writing to distinct fields
    When all three publishers write simultaneously for 100 iterations
    Then every captured snapshot SHALL contain internally consistent field values with no corruption  @AC-139.3
  Scenario: Slow consumer gets dropped tail under backpressure
    Given a telemetry bus with a bounded drop-tail queue of capacity 8
    When 16 items are published before the consumer reads any
    Then the consumer SHALL receive at most 8 items and older items SHALL have been dropped  @AC-139.4
  Scenario: Subscriber receives update within one 250 Hz tick
    Given a telemetry bus running at 250 Hz with one active subscriber
    When a value is published at tick N
    Then the subscriber SHALL observe the new value no later than tick N plus 1  @AC-139.5
  Scenario: Bus marks snapshot invalid after N missed publishes
    Given a telemetry bus configured with a staleness threshold of 5 missed ticks
    When no publisher writes for 6 consecutive ticks
    Then the snapshot SHALL be marked invalid  @AC-139.6
  Scenario: Unsubscribe clears subscriber slot
    Given a telemetry bus with one active subscriber occupying slot 0
    When the subscriber calls unsubscribe
    Then slot 0 SHALL be free and available for a new subscriber
