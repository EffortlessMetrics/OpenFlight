@REQ-405 @product
Feature: Multi-Axis Synchronized Capture — Snapshot All Axis Values Atomically

  @AC-405.1
  Scenario: Synchronized capture captures all active axis values in a single atomic snapshot
    Given multiple active axes
    When a synchronized capture is taken
    Then all axis values SHALL be captured atomically in a single snapshot

  @AC-405.2
  Scenario: Snapshot timestamp is the same for all axes in the same capture
    Given a synchronized capture containing multiple axes
    When the snapshot timestamps are inspected
    Then all axes in the snapshot SHALL share the same timestamp

  @AC-405.3
  Scenario: Snapshot is taken at the start of each RT tick
    Given the RT scheduler running at 250Hz
    When a tick begins
    Then a synchronized axis snapshot SHALL be taken at the start of the tick

  @AC-405.4
  Scenario: Snapshot is accessible to non-RT threads via a lock-free slot
    Given a snapshot taken on the RT thread
    When a non-RT thread reads the snapshot
    Then it SHALL be accessible through a lock-free slot without blocking

  @AC-405.5
  Scenario: Snapshot includes tick_id, timestamp_ns, and values per axis
    Given a synchronized snapshot
    When its fields are inspected
    Then it SHALL contain tick_id, timestamp_ns, and a value for each active axis

  @AC-405.6
  Scenario: Property test — snapshot values are always within [-1.0, 1.0]
    Given any valid synchronized snapshot
    When all axis values are inspected
    Then every value SHALL be within the range [-1.0, 1.0]
