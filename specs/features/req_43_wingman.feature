@req_43 @wingman
Feature: REQ-43 – Project Wingman Adapter

  Background:
    Given the OpenFlight adapter registry is initialised

  @AC-43.1
  Scenario: Process detection identifies Project Wingman
    Given the ProcessDetectionConfig is loaded with defaults
    When the process definitions are inspected
    Then a definition for "Wingman" is present
    And the definition includes process name "ProjectWingman.exe"
    And the definition includes window title "Project Wingman"

  @AC-43.1
  Scenario: Adapter starts in Disconnected state
    Given a WingmanAdapter with default configuration
    Then the adapter state is "Disconnected"

  @AC-43.2
  Scenario: Starting adapter transitions to Connected
    Given a WingmanAdapter with default configuration
    When the adapter is started
    Then the adapter state is "Connected"

  @AC-43.2
  Scenario: Stopping adapter transitions to Disconnected
    Given a WingmanAdapter with default configuration
    And the adapter is started
    When the adapter is stopped
    Then the adapter state is "Disconnected"

  @AC-43.3
  Scenario: Presence snapshot carries correct SimId
    Given a WingmanAdapter with default configuration
    And the adapter is started
    When poll_once is called
    Then the returned snapshot has SimId "Wingman"

  @AC-43.3
  Scenario: Presence snapshot has no valid telemetry
    Given a WingmanAdapter with default configuration
    And the adapter is started
    When poll_once is called
    Then snapshot.validity.attitude_valid is false
    And snapshot.validity.velocities_valid is false
    And snapshot.validity.position_valid is false
    And snapshot.validity.safe_for_ffb is false

  @AC-43.4
  Scenario: Polling without starting returns an error
    Given a WingmanAdapter with default configuration
    When poll_once is called without starting
    Then a NotStarted error is returned

  @AC-43.5
  Scenario: Virtual controller stub stores axis value
    Given a StubVirtualController
    When send_axis is called with index 0 and value 0.75
    Then axis 0 reads back 0.75

  @AC-43.5
  Scenario: Virtual controller stub rejects out-of-range axis
    Given a StubVirtualController
    When send_axis is called with index 8
    Then an AxisOutOfRange error is returned
