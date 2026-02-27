@REQ-579 @product
Feature: Bus Snapshot Schema Version — Bus snapshots should include schema version for forward compatibility  @AC-579.1
  Scenario: BusSnapshot struct includes schema version field
    Given the bus snapshot subsystem is initialized
    When a BusSnapshot is produced
    Then it SHALL contain a schema_version field  @AC-579.2
  Scenario: Schema version increments when snapshot fields are added
    Given a new field is added to the BusSnapshot struct
    When the snapshot schema version is inspected
    Then the schema version SHALL be higher than the previous version  @AC-579.3
  Scenario: Subscribers check schema version before reading fields
    Given a subscriber receives a BusSnapshot
    When the subscriber processes the snapshot
    Then it SHALL check the schema_version field before reading any optional fields  @AC-579.4
  Scenario: Version mismatch triggers a warning log
    Given a subscriber receives a BusSnapshot with a higher schema version than expected
    When the version check is performed
    Then a warning SHALL be logged indicating the schema version mismatch
