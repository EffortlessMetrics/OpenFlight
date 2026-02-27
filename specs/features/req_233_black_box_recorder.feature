@REQ-233 @infra
Feature: Black box recorder captures RT spine events for post-incident analysis  @AC-233.1
  Scenario: Recorder maintains 30 second ring buffer of axis events
    Given the RT spine is running and the black box recorder is enabled
    When axis events are processed continuously
    Then the recorder SHALL retain the most recent 30 seconds of axis events in its ring buffer  @AC-233.2
  Scenario: Ring buffer is pre-allocated with no allocation on hot path
    Given the black box recorder is initialised at service startup
    When the recorder writes axis events during RT spine ticks
    Then no heap allocation SHALL occur on the hot path during recording  @AC-233.3
  Scenario: Ring buffer flushed to disk atomically on crash or panic
    Given the service encounters a panic or unexpected crash
    When the crash handler executes
    Then the in-memory ring buffer SHALL be flushed atomically to a dump file on disk  @AC-233.4
  Scenario: Black box dump readable via flightctl blackbox dump
    Given a dump file exists on disk from a previous crash or manual trigger
    When the user runs flightctl blackbox dump
    Then the contents SHALL be decoded and printed to stdout in human-readable form  @AC-233.5
  Scenario: Dump format is structured binary with timestamp and axis values per tick
    Given a black box dump file
    When the dump is parsed
    Then each record SHALL contain a monotonic timestamp and the axis values captured during that tick  @AC-233.6
  Scenario: Black box write completes within 50 microseconds per tick
    Given the black box recorder is active on the RT spine
    When each 250Hz tick is processed
    Then the time to write one record to the ring buffer SHALL not exceed 50 microseconds
