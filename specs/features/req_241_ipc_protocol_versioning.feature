@REQ-241 @infra
Feature: IPC protobuf schema is versioned and backward compatible  @AC-241.1
  Scenario: Proto files tagged with version in package name
    Given the flight IPC proto source files
    When the package declarations are inspected
    Then each proto file SHALL declare a package name containing a version qualifier such as flight.v1  @AC-241.2
  Scenario: New optional fields added without breaking older clients
    Given a client compiled against an older version of the proto schema
    When the service sends a message containing a new optional field unknown to the client
    Then the client SHALL deserialise the message successfully and ignore the unknown field  @AC-241.3
  Scenario: Removed fields replaced with deprecated markers not deleted
    Given a proto field that is no longer actively used
    When the schema is updated to retire that field
    Then the field SHALL be marked as deprecated and reserved rather than physically deleted  @AC-241.4
  Scenario: Version mismatch between client and service returns explicit error
    Given a client sending a request with an incompatible schema version
    When the service processes the request
    Then the service SHALL return an explicit version mismatch error rather than silently misbehaving  @AC-241.5
  Scenario: Proto schema changes reviewed in CI via buf lint check
    Given a pull request that modifies any proto file in the IPC crate
    When CI runs
    Then the buf lint check SHALL execute and block merge if linting rules are violated  @AC-241.6
  Scenario: Schema changelog maintained in flight-ipc CHANGELOG
    Given changes have been made to the IPC proto schema
    When the changes are committed
    Then an entry describing the schema change SHALL exist in crates/flight-ipc/CHANGELOG.md
