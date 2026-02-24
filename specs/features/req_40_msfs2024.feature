@REQ-40
Feature: MSFS 2024 SimConnect adapter

  @AC-40.1
  Scenario: Adapter reports SimId::Msfs for MSFS 2020 application version
    Given a newly created MsfsAdapter that has not yet connected
    When a Connected event is received with dwApplicationVersionMajor 11
    Then the adapter sim_id SHALL be Msfs

  @AC-40.1
  Scenario: Adapter defaults to SimId::Msfs before connection
    Given a newly created MsfsAdapter that has not yet connected
    When the sim_id is queried before any connection event
    Then the result SHALL be Msfs or Msfs2024

  @AC-40.2
  Scenario: Adapter reports SimId::Msfs2024 for MSFS 2024 application version
    Given a newly created MsfsAdapter that has not yet connected
    When a Connected event is received with dwApplicationVersionMajor 13
    Then the adapter sim_id SHALL be Msfs2024

  @AC-40.2
  Scenario: Application version boundary (12 vs 13)
    Given a newly created MsfsAdapter that has not yet connected
    When a Connected event is received with dwApplicationVersionMajor 12
    Then the adapter sim_id SHALL be Msfs
    When a Connected event is received with dwApplicationVersionMajor 13
    Then the adapter sim_id SHALL be Msfs2024

  @AC-40.3
  Scenario: Correct attitude SimVar names are used in kinematics definition
    Given the default MSFS variable mapping
    When the kinematics data definition is inspected
    Then it SHALL contain "PLANE PITCH DEGREES" and NOT "ATTITUDE PITCH DEGREES"
    And it SHALL contain "PLANE BANK DEGREES" and NOT "ATTITUDE BANK DEGREES"
    And it SHALL contain "PLANE HEADING DEGREES MAGNETIC" and NOT "ATTITUDE HEADING DEGREES"

  @AC-40.4
  Scenario: Lateral G from ACCELERATION BODY X is converted to G units
    Given the default MSFS variable mapping
    When the kinematics data definition is inspected
    Then it SHALL contain "ACCELERATION BODY X" with unit "feet per second squared"
    And the conversion SHALL divide the raw value by 32.174 to yield G units

  @AC-40.4
  Scenario: Longitudinal G from ACCELERATION BODY Z is converted to G units
    Given the default MSFS variable mapping
    When the kinematics data definition is inspected
    Then it SHALL contain "ACCELERATION BODY Z" with unit "feet per second squared"
    And the conversion SHALL divide the raw value by 32.174 to yield G units

  @AC-40.5
  Scenario: Process detection defines Msfs2024 with correct executable
    Given a default ProcessDetectionConfig
    When the definitions are queried for Msfs2024
    Then a definition SHALL exist with process name "FlightSimulator2024.exe"
    And the window title pattern SHALL include "Microsoft Flight Simulator 2024"

  @AC-40.5
  Scenario: Both MSFS variants have separate process definitions
    Given a default ProcessDetectionConfig
    When all simulator definitions are listed
    Then definitions for BOTH Msfs AND Msfs2024 SHALL be present
    And they SHALL have distinct process name patterns
