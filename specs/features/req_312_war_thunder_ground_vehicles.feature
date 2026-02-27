@REQ-312 @product
Feature: War Thunder Ground Vehicles

  @AC-312.1
  Scenario: Service reads War Thunder ground vehicle telemetry
    Given War Thunder is running with a ground vehicle session active
    When the service is connected to the War Thunder telemetry interface
    Then the service SHALL read ground vehicle telemetry data from the War Thunder HTTP telemetry endpoint

  @AC-312.2
  Scenario: Ground vehicle profiles can define throttle and steering axes
    Given a War Thunder ground vehicle profile is loaded
    When the profile is parsed
    Then the profile SHALL allow axis bindings for throttle and steering controls specific to ground vehicles

  @AC-312.3
  Scenario: Tank turret angle is available as a virtual axis
    Given the service is receiving War Thunder ground vehicle telemetry
    When turret angle data is present in the telemetry
    Then the service SHALL expose the tank turret angle as a virtual axis available for profile bindings

  @AC-312.4
  Scenario: Combat mode detection switches profile variant
    Given the service is monitoring War Thunder ground vehicle state
    When the combat mode changes (e.g. arcade vs realistic vs simulator)
    Then the service SHALL detect the combat mode and switch to the corresponding profile variant

  @AC-312.5
  Scenario: Both aircraft and ground vehicle can be active simultaneously
    Given the service has profiles configured for both aircraft and ground vehicles
    When the user switches between aircraft and ground vehicle sessions in War Thunder
    Then the service SHALL support both profile types being configured simultaneously and activate the appropriate one

  @AC-312.6
  Scenario: Profile contains separate section for ground vs air
    Given a War Thunder profile definition
    When the profile schema is validated
    Then the profile SHALL support separate configuration sections for ground vehicle controls and aircraft controls
