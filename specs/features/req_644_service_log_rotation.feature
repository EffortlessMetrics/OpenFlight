Feature: Service Log Rotation
  As a flight simulation enthusiast
  I want the service to support automatic log file rotation
  So that log files do not grow unbounded and disk space is managed automatically

  Background:
    Given the OpenFlight service is running with log rotation configured

  Scenario: Log files rotate when size exceeds configurable maximum
    Given the maximum log file size is configured to 10 MB
    When the current log file reaches 10 MB in size
    Then the log file is rotated and a new log file is started

  Scenario: Configurable number of rotated log files are retained
    Given the log retention count is configured to 5
    When more than 5 log rotation events have occurred
    Then only the 5 most recent rotated log files are retained on disk

  Scenario: Log rotation does not interrupt service operation
    Given the service is actively processing axis inputs
    When a log rotation event occurs
    Then the service continues processing without interruption

  Scenario: Log path and rotation config are in service config file
    When the service config file is inspected
    Then it contains fields for log path, maximum log size, and retention count
