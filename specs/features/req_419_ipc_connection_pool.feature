@REQ-419 @infra
Feature: IPC Connection Pool — Support Multiple Simultaneous CLI Connections

  @AC-419.1
  Scenario: Service IPC server supports up to 10 simultaneous client connections
    Given the IPC server with default configuration
    When 10 clients connect simultaneously
    Then all 10 connections SHALL be accepted and served

  @AC-419.2
  Scenario: Connections beyond the pool limit are rejected with a BusyError
    Given the connection pool is at capacity (10 connections)
    When an eleventh client attempts to connect
    Then the connection SHALL be rejected with a BusyError

  @AC-419.3
  Scenario: Connection pool is cleaned up when clients disconnect
    Given a full connection pool
    When a client disconnects
    Then the freed slot SHALL become available for new connections immediately

  @AC-419.4
  Scenario: Pool size is configurable in service configuration
    Given a service configuration specifying a custom max_connections value
    When the service starts
    Then the IPC server SHALL enforce the configured maximum connection count

  @AC-419.5
  Scenario: Current connection count is exposed via metrics
    Given the metrics endpoint
    When clients are connected
    Then the ipc_active_connections metric SHALL reflect the current connection count

  @AC-419.6
  Scenario: Connection pool is covered by an integration test with multiple clients
    Given an integration test harness
    When multiple concurrent clients connect and issue requests
    Then all requests SHALL be handled correctly without race conditions or panics
