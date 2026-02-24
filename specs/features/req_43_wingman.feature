@req_43 @wingman
Feature: REQ-43 – Project Wingman Adapter

  Background:
    Given the OpenFlight adapter registry is initialised

  Scenario: Process detection identifies Project Wingman
    Given the ProcessDetectionConfig is loaded with defaults
    When the process definitions are inspected
    Then a definition for "Wingman" is present
    And the definition includes process name "ProjectWingman.exe"
    And the definition includes window title "Project Wingman"

  Scenario: Adapter starts in Disconnected state
    Given a WingmanAdapter with default configuration
    Then the adapter state is "Disconnected"

  Scenario: Starting adapter transitions to Connected
    Given a WingmanAdapter with default configuration
    When the adapter is started
    Then the adapter state is "Connected"

  Scenario: Stopping adapter transitions to Disconnected
    Given a WingmanAdapter with default configuration
    And the adapter is started
    When the adapter is stopped
    Then the adapter state is "Disconnected"

  Scenario: Presence snapshot carries correct SimId
    Given a WingmanAdapter with default configuration
    And the adapter is started
    When poll_once is called
    Then the returned snapshot has SimId "Wingman"

  Scenario: Presence snapshot has no valid telemetry
    Given a WingmanAdapter with default configuration
    And the adapter is started
    When poll_once is called
    Then snapshot.validity.attitude_valid is false
    And snapshot.validity.velocities_valid is false
    And snapshot.validity.position_valid is false
    And snapshot.validity.safe_for_ffb is false

  Scenario: Polling without starting returns an error
    Given a WingmanAdapter with default configuration
    When poll_once is called without starting
    Then a NotStarted error is returned

  Scenario: Virtual controller stub stores axis value
    Given a StubVirtualController
    When send_axis is called with index 0 and value 0.75
    Then axis 0 reads back 0.75

  Scenario: Virtual controller stub rejects out-of-range axis
    Given a StubVirtualController
    When send_axis is called with index 8
    Then an AxisOutOfRange error is returned
