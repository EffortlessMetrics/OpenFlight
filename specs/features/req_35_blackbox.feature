@REQ-35
Feature: Blackbox Recording System

  Background:
    Given the blackbox recording system is available

  @AC-35.1
  Scenario: Default config values are applied and queue capacity clamps
    Given a new BlackboxConfig is created with defaults
    Then the default flush interval should be set
    And the record queue capacity should be clamped to the minimum bound when below minimum
    And the record queue capacity should be clamped to the maximum bound when above maximum

  @AC-35.2
  Scenario: BlackboxWriter lifecycle — start state and double-start guard
    Given a BlackboxWriter is constructed
    Then it should start in a non-running state
    When the writer is started a second time
    Then it should return an error indicating it is already running

  @AC-35.3
  Scenario: Round-trip write and read back records
    Given a running BlackboxWriter
    When records are written to the blackbox
    Then those records should be readable in the correct order

  @AC-35.4
  Scenario: Stream type index is stable
    Given the set of blackbox stream types
    When stream_type_to_index is called for each type
    Then it should return a stable monotonic index value

  @AC-35.5
  Scenario Outline: Header, footer, and index entry round-trip serialization
    Given an arbitrary <structure> with random valid field values
    When it is serialized and deserialized
    Then all fields should be preserved exactly

    Examples:
      | structure     |
      | header        |
      | footer        |
      | index entry   |
