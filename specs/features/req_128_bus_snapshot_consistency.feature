@REQ-128 @infra
Feature: Bus snapshot consistency

  @AC-128.1
  Scenario: Snapshot timestamp is monotonic
    Given a flight-bus producing snapshots at 250 Hz
    When a series of consecutive snapshots is collected
    Then each snapshot's timestamp SHALL be greater than or equal to the previous snapshot's timestamp
    And no timestamp regression SHALL occur

  @AC-128.2
  Scenario: Snapshot from DCS export contains expected fields
    Given a bus snapshot originating from the DCS export adapter
    When the snapshot is inspected
    Then it SHALL contain pitch, roll, and heading fields
    And it SHALL contain an airspeed field
    And the source identifier SHALL indicate the DCS adapter

  @AC-128.3
  Scenario: Snapshot from War Thunder contains expected fields
    Given a bus snapshot originating from the War Thunder telemetry adapter
    When the snapshot is inspected
    Then it SHALL contain pitch, roll, and heading fields
    And it SHALL contain an indicated airspeed field
    And the source identifier SHALL indicate the War Thunder adapter

  @AC-128.4
  Scenario: Stale snapshot marked invalid after timeout
    Given a bus snapshot produced more than the configured staleness timeout ago
    When the snapshot validity is checked
    Then the snapshot SHALL be marked as invalid
    And subscribers SHALL be notified that the data is stale

  @AC-128.5
  Scenario: Subscriber receives snapshot within one tick period
    Given a subscriber registered on the flight-bus
    When the RT spine produces a new snapshot at a tick boundary
    Then the subscriber SHALL receive the snapshot within one 4 ms tick period

  @AC-128.6
  Scenario: Multiple subscribers see the same snapshot data
    Given three independent subscribers registered on the flight-bus
    When the RT spine publishes a single snapshot
    Then all three subscribers SHALL receive an identical copy of the snapshot
    And no subscriber SHALL observe different field values from the same snapshot
