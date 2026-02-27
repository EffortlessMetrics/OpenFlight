@REQ-414 @product
Feature: OpenFlight Telemetry Bus Schema — Define Canonical Bus Event Types

  @AC-414.1
  Scenario: BusSnapshot struct includes required fields
    Given the BusSnapshot struct definition
    When its fields are inspected
    Then it SHALL include at minimum: tick_id, timestamp_ns, axes, and buttons

  @AC-414.2
  Scenario: BusSnapshot is versioned with a schema_version field
    Given a BusSnapshot instance
    When its fields are inspected
    Then it SHALL contain a schema_version field

  @AC-414.3
  Scenario: BusSnapshot serialization is zero-copy using bytemuck or similar
    Given a BusSnapshot being serialized for transmission
    When serialization is performed
    Then it SHALL be zero-copy, using bytemuck or an equivalent zero-copy mechanism

  @AC-414.4
  Scenario: BusSnapshot size is fixed with no variable-length fields on RT path
    Given the BusSnapshot struct
    When its memory layout is inspected
    Then it SHALL be a fixed-size type with no variable-length fields on the RT path

  @AC-414.5
  Scenario: BusSnapshot schema is documented in docs/reference/bus-schema.md
    Given the docs/reference/bus-schema.md file
    When it is inspected
    Then it SHALL document the BusSnapshot schema fields and their semantics

  @AC-414.6
  Scenario: Property test — BusSnapshot serialization round-trips correctly
    Given any valid BusSnapshot
    When it is serialized and then deserialized
    Then the result SHALL equal the original BusSnapshot
