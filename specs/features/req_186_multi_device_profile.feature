@REQ-186 @product
Feature: Multiple devices can be assigned to different profile roles simultaneously  @AC-186.1
  Scenario: Profile binds multiple device types to named roles
    Given a profile with role bindings for primary_stick, secondary_stick, and rudder_pedals
    When the profile is loaded and matching devices are connected
    Then each device type SHALL be bound to its named role and available for axis mapping  @AC-186.2
  Scenario: Primary stick takes priority over secondary stick
    Given a profile with both primary_stick and secondary_stick roles bound to connected devices
    When the same axis input is produced on both devices simultaneously
    Then the primary_stick device SHALL take priority and its axis value SHALL be used  @AC-186.3
  Scenario: Device role assignments survive device reconnect
    Given a device assigned to a role in an active profile is disconnected and reconnected
    When the device is re-enumerated by the HID subsystem
    Then the device SHALL be reassigned to its original role without requiring a profile reload  @AC-186.4
  Scenario: Conflict detected when two devices assigned to same role
    Given a profile where two devices are assigned to the same named role
    When the profile is validated or loaded
    Then a conflict warning SHALL be emitted identifying the duplicate role assignment  @AC-186.5
  Scenario: Profile migration handles device role field changes
    Given a profile persisted with a previous schema version that added or removed device role fields
    When the profile is migrated to the current schema version
    Then the migration SHALL succeed and preserve all role assignments that remain valid  @AC-186.6
  Scenario: CLI shows current device-to-role assignments
    Given one or more devices are bound to roles under an active profile
    When the user runs the flightctl status command
    Then the output SHALL include the current device-to-role assignments for all active roles
