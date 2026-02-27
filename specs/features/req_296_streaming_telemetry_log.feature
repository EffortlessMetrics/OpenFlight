@REQ-296 @product
Feature: Streaming Telemetry Log  @AC-296.1
  Scenario: Service writes axis values to rolling binary log at 10Hz
    Given streaming telemetry logging is enabled in the service configuration
    When the service runs for 30 seconds
    Then the rolling binary log SHALL contain axis value records sampled at 10Hz  @AC-296.2
  Scenario: Log format is versioned and documented
    Given a telemetry log file written by the service
    When the file header is inspected
    Then it SHALL contain a version field matching the documented log format version  @AC-296.3
  Scenario: Log includes timestamps, device IDs, and raw and processed values
    Given a telemetry log with at least one recorded entry
    When the entry is decoded
    Then it SHALL contain a monotonic timestamp, the originating device ID, the raw axis value, and the post-processing axis value  @AC-296.4
  Scenario: CLI can tail the log in real-time
    Given the service is running with telemetry logging enabled
    When the command "flightctl log -f" is run
    Then the CLI SHALL stream new log entries to stdout as they are written, similar to tail -f  @AC-296.5
  Scenario: Log retention is configurable with a default of five minutes
    Given the service is running with default log retention settings
    When more than five minutes of telemetry data have been written
    Then records older than five minutes SHALL be evicted from the rolling log  @AC-296.6
  Scenario: Log can be exported to CSV for analysis
    Given a telemetry log containing axis records
    When the command "flightctl log --export csv" is run
    Then the output SHALL be valid CSV with columns for timestamp, device ID, axis ID, raw value, and processed value
