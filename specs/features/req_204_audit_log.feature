@REQ-204 @infra
Feature: Security-relevant events written to tamper-evident audit log  @AC-204.1
  Scenario: Profile changes logged with timestamp and source
    Given the audit log is active
    When a profile change is made via the API with source identified as user
    Then an audit log entry SHALL be written with timestamp and source type  @AC-204.2
  Scenario: Service start and stop events logged with process ID
    Given the service is starting
    When the service process begins
    Then an audit log entry SHALL be written containing the service start event and process ID  @AC-204.3
  Scenario: Device connect and disconnect events logged with USB VID and PID
    Given a USB HID device is plugged in
    When the device connection is detected
    Then an audit log entry SHALL be written containing the USB VID and PID  @AC-204.4
  Scenario: Audit log stored separately from application logs
    Given both audit logging and application logging are active
    When audit events and application log events are generated
    Then audit entries SHALL reside in a separate file or store from application log entries  @AC-204.5
  Scenario: Audit log entries are append-only with no delete API
    Given audit log entries have been written
    When a delete request is issued against the audit log
    Then the request SHALL be rejected and no entries SHALL be removed  @AC-204.6
  Scenario: Audit log viewable via flightctl audit tail command
    Given audit log entries exist
    When the user runs flightctl audit tail
    Then the most recent audit entries SHALL be displayed in the terminal
