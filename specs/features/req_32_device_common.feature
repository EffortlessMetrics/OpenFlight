@REQ-32
Feature: Device common types, health states, and metrics

  @AC-32.1
  Scenario: VID/PID formats as expected hex string
    Given a device with VID 0x231D and PID 0x0200
    When the VID/PID is formatted as a string
    Then the result SHALL use the expected hex format

  @AC-32.1
  Scenario: Virtual device builder constructs a device
    Given a virtual device builder with name and type parameters
    When the builder is finalized
    Then the resulting device SHALL have the expected fields set

  @AC-32.2
  Scenario: Operational state transitions are validated
    Given a device in the Idle state
    When valid and invalid state transitions are attempted
    Then valid transitions SHALL succeed
    And invalid transitions SHALL be rejected

  @AC-32.3
  Scenario: Operation totals are updated on record
    Given a device metrics instance
    When operations are recorded
    Then total operation counts SHALL be incremented correctly

  @AC-32.3
  Scenario: Operation metrics update the shared registry
    Given a device metrics instance with a metrics registry
    When operations are recorded
    Then the metrics registry SHALL reflect the updated counts
