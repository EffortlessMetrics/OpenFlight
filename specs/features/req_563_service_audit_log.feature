Feature: Service Audit Log
  As a flight simulation enthusiast
  I want the service to maintain an audit log of configuration changes
  So that I can review what was changed, when, and by whom

  Background:
    Given the OpenFlight service is running with audit logging enabled

  Scenario: Profile changes are written to audit log with timestamp and operator
    When the operator applies a new profile via "flightctl profile apply cessna172.toml"
    Then an audit log entry is written containing the timestamp, operator name, and profile name

  Scenario: Audit log survives service restarts
    Given an audit log entry was written before the service was stopped
    When the service is restarted
    Then the previous audit log entry is still present in the log file

  Scenario: Audit log is viewable via CLI
    Given at least one audit log entry exists
    When the operator runs "flightctl audit log"
    Then the CLI prints the audit log entries in reverse chronological order

  Scenario: Audit log has configurable retention period
    Given the audit log retention is configured to 7 days
    When an audit entry is older than 7 days
    Then the service removes that entry during the next log rotation
