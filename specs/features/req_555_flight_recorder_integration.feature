@REQ-555 @product
Feature: Flight Recorder Integration — Service should integrate with MSFS flight recorder

  @AC-555.1
  Scenario: Service can trigger MSFS flight recorder start/stop via SimConnect event
    Given the SimConnect adapter is connected to MSFS
    When the operator issues the start-flight-recording CLI command
    Then the service SHALL send the appropriate SimConnect event to start the MSFS flight recorder

  @AC-555.2
  Scenario: OpenFlight axis log can be time-synchronized with MSFS recording
    Given both OpenFlight axis recording and MSFS flight recorder are active
    When a synchronization timestamp is captured at recording start
    Then the OpenFlight log SHALL contain the synchronization timestamp for alignment

  @AC-555.3
  Scenario: Synchronized log export is available via CLI
    Given a completed synchronized recording session
    When the export-synchronized-log CLI command is issued
    Then the service SHALL produce an export file containing both OpenFlight and MSFS timing data

  @AC-555.4
  Scenario: Recording metadata includes software version
    Given a recording has been created
    When the recording metadata is inspected
    Then it SHALL include the OpenFlight software version string
