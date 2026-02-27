@REQ-299 @product
Feature: Audit Log  @AC-299.1
  Scenario: All profile changes are recorded in an audit log
    Given the service is running with audit logging enabled
    When a profile is created, modified, or deleted
    Then the change SHALL be recorded as an entry in the audit log  @AC-299.2
  Scenario: Audit log includes timestamp user and change description
    Given an audit log entry exists for a profile change
    When the entry is read
    Then it SHALL contain a UTC timestamp, the identity of the acting user or process, and a human-readable change description  @AC-299.3
  Scenario: Audit log survives service restarts
    Given the service has written audit log entries
    When the service is stopped and restarted
    Then previously written audit log entries SHALL still be present and readable  @AC-299.4
  Scenario: CLI can display audit log via flightctl log --audit
    Given the service has recorded audit log entries
    When the command "flightctl log --audit" is run
    Then the CLI SHALL display the audit log entries in chronological order  @AC-299.5
  Scenario: Audit log max size is configurable with a default of 10MB rolling
    Given the audit log has grown to the configured maximum size
    When a new entry is appended
    Then the oldest entries SHALL be evicted to maintain the size limit with a default limit of 10MB  @AC-299.6
  Scenario: Sensitive data is never written to the audit log
    Given a profile change involves a field containing a password or authentication token
    When the audit log entry is written
    Then the sensitive field value SHALL be redacted or omitted from the log entry
