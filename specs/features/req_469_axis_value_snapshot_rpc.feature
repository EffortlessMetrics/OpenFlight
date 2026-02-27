@REQ-469 @product
Feature: Axis Value Snapshot RPC — gRPC GetAxisSnapshot Endpoint  @AC-469.1
  Scenario: GetAxisSnapshot returns processed values for all axes
    Given the service is running with three active virtual axes
    When a GetAxisSnapshot RPC is called
    Then the response SHALL contain the current processed value for each of the three axes  @AC-469.2
  Scenario: Snapshot includes pipeline stage values for diagnostics
    Given a virtual axis with deadzone, expo, and smoothing stages configured
    When a GetAxisSnapshot RPC is called
    Then the response SHALL include per-stage values (raw, post-deadzone, post-expo, post-smooth)  @AC-469.3
  Scenario: Snapshot RPC responds within 10ms
    Given the service is running under normal load
    When a GetAxisSnapshot RPC is issued
    Then the response SHALL be received within 10 milliseconds  @AC-469.4
  Scenario: Snapshot includes device ID and axis name for each value
    Given multiple devices with axes are active
    When a GetAxisSnapshot RPC is called
    Then each axis entry in the response SHALL include the source device ID and axis name
