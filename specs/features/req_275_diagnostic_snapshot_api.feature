@REQ-275 @product
Feature: Diagnostic snapshot API exposes axis values, devices, active profile, and pipeline stages  @AC-275.1
  Scenario: gRPC endpoint returns current axis values snapshot
    Given the service is running with at least one axis actively producing values
    When a client calls the diagnostic snapshot gRPC endpoint
    Then the response SHALL contain the current normalised value for each active axis  @AC-275.2
  Scenario: Snapshot includes connected device list
    Given one or more HID devices are connected and recognised by the service
    When a client calls the diagnostic snapshot gRPC endpoint
    Then the response SHALL list each connected device with its VID, PID, and display name  @AC-275.3
  Scenario: Snapshot includes active profile name
    Given the service has a profile loaded
    When a client calls the diagnostic snapshot gRPC endpoint
    Then the response SHALL include the name of the currently active profile  @AC-275.4
  Scenario: Snapshot includes axis pipeline stage values
    Given an axis is processing input through deadzone and curve stages
    When a client calls the diagnostic snapshot gRPC endpoint
    Then the response SHALL include per-axis values at each pipeline stage including pre-deadzone, post-deadzone, and post-curve  @AC-275.5
  Scenario: Snapshot is idempotent for unchanged state
    Given the service state has not changed between two consecutive calls
    When the diagnostic snapshot gRPC endpoint is called twice in succession
    Then both responses SHALL be identical  @AC-275.6
  Scenario: CLI can display snapshot in table format
    Given the service is running
    When the user runs the CLI snapshot command
    Then the CLI SHALL render the snapshot data as a formatted table in the terminal
