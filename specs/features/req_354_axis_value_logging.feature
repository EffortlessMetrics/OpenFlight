@REQ-354 @blackbox @axis @logging
Feature: Axis value logging to flight recorder
  As a user analysing flight control behaviour
  I want all processed axis values recorded to the blackbox
  So that I can replay and diagnose input anomalies after the fact

  Scenario: Axis values logged at blackbox sample rate  @AC-354.1
    Given the blackbox is configured with a sample rate of 50 Hz
    When the service runs for 1 second
    Then the blackbox SHALL contain approximately 50 axis log entries per axis

  Scenario: Each log entry contains required fields  @AC-354.2
    Given the blackbox is enabled and the service is running
    When a log entry is written
    Then it SHALL contain tick count, axis ID, raw value, and processed value

  Scenario: Logging overhead is within budget  @AC-354.3
    Given 100 axes are active and blackbox logging is enabled
    When the tick processing time is measured over 1000 ticks
    Then the logging overhead SHALL be under 5 us per tick on average

  Scenario: Log entries written without heap allocation  @AC-354.4
    Given the blackbox ring buffer is pre-allocated
    When axis log entries are written during a tick
    Then no heap allocation SHALL occur during the write operation

  Scenario: Blackbox replay reconstructs axis history  @AC-354.5
    Given a recording session has been captured
    When the blackbox replay tool processes the recording
    Then it SHALL be able to reconstruct the complete sequence of axis values

  Scenario: Logging can be disabled per-axis  @AC-354.6
    Given axis "rudder" has blackbox logging disabled in its configuration
    When the service runs and the rudder axis is active
    Then no log entries for "rudder" SHALL appear in the blackbox output
