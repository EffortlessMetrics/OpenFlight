@REQ-443 @product
Feature: Telemetry Replay Mode — Replay Recorded Telemetry for Testing

  @AC-443.1
  Scenario: Replay mode loads a recorded trace file and plays it back
    Given a valid trace file recorded by the blackbox
    When replay mode is started with that trace file
    Then the service SHALL begin publishing snapshots from the trace to the bus

  @AC-443.2
  Scenario: Replay speed is configurable from 0.1x to 10x
    Given replay mode is active
    When a playback speed of 2.0x is configured
    Then snapshots SHALL be published at twice the original recording rate

  @AC-443.3
  Scenario: Replay publishes snapshots to the bus identically to live data
    Given a trace file captured from a live session
    When the same trace is replayed
    Then each snapshot published during replay SHALL have the same field values as the original recording

  @AC-443.4
  Scenario: Replay loops or stops at end of trace according to configuration
    Given replay mode is configured with loop: false
    When the end of the trace file is reached
    Then replay SHALL stop and the service SHALL return to idle state
