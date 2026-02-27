@REQ-550 @product
Feature: Axis Chain Snapshot RPC — gRPC service should expose axis chain snapshot endpoint

  @AC-550.1
  Scenario: GetAxisSnapshot RPC returns current value at each pipeline stage
    Given the gRPC service is running
    When a GetAxisSnapshot request is issued for a specific axis
    Then the response SHALL contain the current value at each stage of the axis pipeline

  @AC-550.2
  Scenario: Snapshot includes timestamps for each stage
    Given a GetAxisSnapshot response
    When the response is inspected
    Then each pipeline stage entry SHALL include a monotonic timestamp

  @AC-550.3
  Scenario: Snapshot is available without enabling debug mode
    Given the service is running in production mode with debug mode disabled
    When a GetAxisSnapshot RPC is called
    Then the RPC SHALL succeed and return valid snapshot data

  @AC-550.4
  Scenario: Snapshot latency does not affect RT processing
    Given the RT spine is running at 250Hz
    When a GetAxisSnapshot RPC is in progress
    Then the RT spine tick jitter SHALL remain within the QG-RT-JITTER budget
