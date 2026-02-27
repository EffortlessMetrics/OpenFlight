Feature: Log Rotation
  As a flight simulation enthusiast
  I want log rotation
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Log files rotate automatically when size exceeds configured threshold
    Given the system is configured for log rotation
    When the feature is exercised
    Then log files rotate automatically when size exceeds configured threshold

  Scenario: Rotated log files are compressed and retained for configurable duration
    Given the system is configured for log rotation
    When the feature is exercised
    Then rotated log files are compressed and retained for configurable duration

  Scenario: Log rotation occurs without dropping any log entries during switchover
    Given the system is configured for log rotation
    When the feature is exercised
    Then log rotation occurs without dropping any log entries during switchover

  Scenario: Maximum total log storage is bounded by configurable disk quota
    Given the system is configured for log rotation
    When the feature is exercised
    Then maximum total log storage is bounded by configurable disk quota
