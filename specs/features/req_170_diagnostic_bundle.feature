@REQ-170 @product
Feature: Diagnostic bundle export

  @AC-170.1
  Scenario: Bundle contains current profile snapshot
    Given the system is running with an active profile
    When a diagnostic bundle is exported
    Then the bundle SHALL contain a snapshot of the currently active profile

  @AC-170.2
  Scenario: Bundle contains device list
    Given one or more devices are connected and active
    When a diagnostic bundle is exported
    Then the bundle SHALL contain the list of all connected devices with their identifiers and status

  @AC-170.3
  Scenario: Bundle contains last 1000 trace events
    Given the tracing subsystem has recorded trace events
    When a diagnostic bundle is exported
    Then the bundle SHALL contain at most the last 1000 trace events in chronological order

  @AC-170.4
  Scenario: Bundle contains health check results
    Given the health check subsystem is active
    When a diagnostic bundle is exported
    Then the bundle SHALL contain the results of the most recent health checks for all subsystems

  @AC-170.5
  Scenario: Bundle is exportable as ZIP
    Given the diagnostic bundle has been assembled
    When the export is requested
    Then the bundle SHALL be written as a valid ZIP archive to the specified output path

  @AC-170.6
  Scenario: Bundle timestamp is ISO 8601
    Given a diagnostic bundle is exported
    When the bundle metadata is inspected
    Then the bundle timestamp SHALL be formatted as a valid ISO 8601 date-time string

  @AC-170.7
  Scenario: Bundle exported on safe-mode trigger
    Given the system enters safe mode due to a fault
    When safe mode is activated
    Then a diagnostic bundle SHALL be automatically exported before safe-mode isolation takes effect
