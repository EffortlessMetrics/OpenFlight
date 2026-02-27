@REQ-379 @product
Feature: Prepar3D v5/v6 Integration via SimConnect  @AC-379.1
  Scenario: SimConnect DLL is loaded dynamically at runtime for P3D
    Given Prepar3D is installed and its SimConnect.dll is present
    When the P3D adapter initializes
    Then the SimConnect DLL SHALL be loaded dynamically without static linking  @AC-379.2
  Scenario: Flight state data is extracted and published to the event bus
    Given a running Prepar3D session with an active flight
    When the P3D adapter receives a SimConnect data frame
    Then position, attitude, and engine data SHALL be published to the event bus  @AC-379.3
  Scenario: P3D adapter shares 80% code with MSFS adapter via adapter-common
    Given the P3D and MSFS SimConnect adapters in flight-adapter-common
    When the shared code ratio is measured
    Then at least 80% of adapter logic SHALL reside in flight-adapter-common  @AC-379.4
  Scenario: P3D game manifest lists supported versions
    Given the P3D adapter game manifest file
    When supported versions are queried
    Then v4, v5, and v6 SHALL be listed as supported versions  @AC-379.5
  Scenario: P3D-specific weather and time queries are handled gracefully
    Given a P3D session with weather and time APIs active
    When the adapter issues P3D-specific weather or time queries
    Then the adapter SHALL process the responses without errors or panics  @AC-379.6
  Scenario: Disconnect and reconnect cycles are covered by integration tests
    Given an integration test simulating P3D SimConnect disconnect and reconnect
    When the connection drops and is re-established
    Then the adapter SHALL reconnect automatically and resume data publishing
