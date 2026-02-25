@REQ-41
Feature: Kerbal Space Program kRPC adapter

  @AC-41.1
  Scenario: Adapter reports SimId::Ksp
    Given a newly created KspAdapter with default configuration
    When the sim_id is queried
    Then the result SHALL be Ksp

  @AC-41.1
  Scenario: Adapter starts in Disconnected state
    Given a newly created KspAdapter with default configuration
    When the adapter state is queried before calling start
    Then the state SHALL be Disconnected

  @AC-41.2
  Scenario: KSP heading is normalized from 0-360 to -180 to +180
    Given a KSP telemetry payload with heading_deg 270.0
    When the telemetry is mapped to a BusSnapshot
    Then kinematics.heading SHALL be -90.0 degrees

  @AC-41.2
  Scenario: KSP heading below 180 is unchanged
    Given a KSP telemetry payload with heading_deg 90.0
    When the telemetry is mapped to a BusSnapshot
    Then kinematics.heading SHALL be 90.0 degrees

  @AC-41.3
  Scenario: Altitude is converted from metres to feet
    Given a KSP telemetry payload with altitude_m 1000.0
    When the telemetry is mapped to a BusSnapshot
    Then environment.altitude SHALL be approximately 3280.84 feet

  @AC-41.3
  Scenario: Speed is converted from m/s to knots
    Given a KSP telemetry payload with speed_mps 100.0
    When the telemetry is mapped to a BusSnapshot
    Then kinematics.tas SHALL be approximately 194.4 knots

  @AC-41.4
  Scenario: safe_for_ffb is true only when flying in atmosphere
    Given a KSP telemetry payload with situation FLYING (3)
    When the telemetry is mapped to a BusSnapshot
    Then validity.safe_for_ffb SHALL be true

  @AC-41.4
  Scenario: safe_for_ffb is false when landed
    Given a KSP telemetry payload with situation LANDED (0)
    When the telemetry is mapped to a BusSnapshot
    Then validity.safe_for_ffb SHALL be false

  @AC-41.5
  Scenario: ProcessDetectionConfig contains KSP definition
    Given the default ProcessDetectionConfig
    When the SimId::Ksp definition is retrieved
    Then it SHALL include process name KSP_x64.exe for Windows
    And it SHALL include process name KSP.x86_64 for Linux
    And it SHALL include window title Kerbal Space Program
