@REQ-225 @infra
Feature: Recorded telemetry sessions can be replayed for testing and debugging  @AC-225.1
  Scenario: Telemetry recorder captures all bus events with microsecond timestamps
    Given the OpenFlight service is running with telemetry recording enabled
    When bus events are emitted during a flight session
    Then all events SHALL be captured with microsecond-precision timestamps in the recording  @AC-225.2
  Scenario: Recorded session stored as compact binary file under size limit
    Given a 60-second telemetry recording has completed
    When the recording is written to disk
    Then the resulting binary file SHALL be less than 1MB in size  @AC-225.3
  Scenario: Replay feeds recorded events into bus at original timing
    Given a telemetry recording file exists
    When flightctl replay is invoked with that file
    Then events SHALL be injected into the event bus at the same relative timing as the original recording  @AC-225.4
  Scenario: Replay can run at accelerated speed for fast debugging
    Given a telemetry recording file exists
    When flightctl replay is invoked with --speed 2 or --speed 4
    Then events SHALL be replayed at 2x or 4x the original speed respectively  @AC-225.5
  Scenario: CLI commands control recording and playback
    Given the OpenFlight service is running
    When the user runs flightctl record to start recording and flightctl replay to play back
    Then recording SHALL start on the record command and playback SHALL start on the replay command  @AC-225.6
  Scenario: Replay session generates same test outcomes as live session
    Given a known telemetry recording with deterministic inputs
    When the recording is replayed
    Then the resulting test outcomes SHALL match those produced by the original live session
