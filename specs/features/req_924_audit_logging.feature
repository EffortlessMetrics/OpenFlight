Feature: Audit Logging
  As a flight simulation enthusiast
  I want audit logging
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Security-relevant events are logged to tamper-evident audit log
    Given the system is configured for audit logging
    When the feature is exercised
    Then security-relevant events are logged to tamper-evident audit log

  Scenario: Audit entries include timestamp, actor, action, and outcome fields
    Given the system is configured for audit logging
    When the feature is exercised
    Then audit entries include timestamp, actor, action, and outcome fields

  Scenario: Audit log rotation preserves completed log files with integrity hashes
    Given the system is configured for audit logging
    When the feature is exercised
    Then audit log rotation preserves completed log files with integrity hashes

  Scenario: Configuration changes, auth events, and plugin loads are audit-logged
    Given the system is configured for audit logging
    When the feature is exercised
    Then configuration changes, auth events, and plugin loads are audit-logged
