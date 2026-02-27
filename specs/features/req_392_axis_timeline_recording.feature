@REQ-392 @product
Feature: Axis Timeline Recording — Capture Axis Values with High-Resolution Timestamps

  @AC-392.1
  Scenario: Recording captures axis ID, value, and monotonic timestamp
    Given axis timeline recording is enabled
    When an axis value is processed on the RT thread
    Then the record SHALL contain the axis ID, value, and a monotonic nanosecond timestamp

  @AC-392.2
  Scenario: Recording buffer is a lock-free ring buffer with configurable capacity
    Given axis timeline recording is configured
    When the recording buffer is inspected
    Then it SHALL be a lock-free ring buffer with capacity set from configuration

  @AC-392.3
  Scenario: Recording can be started and stopped without stopping the RT loop
    Given the RT loop is running
    When recording is started or stopped via flightctl
    Then the RT loop SHALL continue operating without interruption

  @AC-392.4
  Scenario: Recorded data is exported as CSV via flightctl
    Given axis timeline data has been recorded
    When the user runs `flightctl axis export --format csv`
    Then the recorded data SHALL be exported as a valid CSV file

  @AC-392.5
  Scenario: CSV output includes the correct header row
    Given a CSV export of axis timeline data
    When the header row is inspected
    Then it SHALL contain the columns: axis_id, timestamp_ns, raw, processed

  @AC-392.6
  Scenario: Recording overhead is under 1 µs per tick when active
    Given axis timeline recording is active
    When the per-tick overhead is measured
    Then the overhead SHALL be less than 1 µs per tick
