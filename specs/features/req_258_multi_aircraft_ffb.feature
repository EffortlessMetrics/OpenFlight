@REQ-258 @product
Feature: FFB profiles customized per aircraft type for authentic force feel  @AC-258.1
  Scenario: Aircraft-specific FFB profile selects spring and damper gains
    Given a profile file containing an FFB section for a specific aircraft type
    When that aircraft is detected by the sim adapter
    Then the FFB engine SHALL load the aircraft-specific spring and damper gains from the profile  @AC-258.2
  Scenario: Fighter aircraft profile applies heavier spring than trainer
    Given FFB profiles defined for a fighter aircraft and a trainer aircraft
    When the spring gain values are compared
    Then the fighter aircraft profile SHALL have a higher spring gain than the trainer aircraft profile  @AC-258.3
  Scenario: FFB profile applied within 100ms of aircraft type detection
    Given the FFB engine is running and a new aircraft type is detected
    When the aircraft type event arrives on the bus
    Then the new FFB profile SHALL be fully applied within 100 milliseconds of the detection event  @AC-258.4
  Scenario: Trim position reflected in FFB center offset
    Given an aircraft with a non-zero trim setting
    When the trim value is published to the bus
    Then the FFB center offset SHALL be updated to reflect the current trim position  @AC-258.5
  Scenario: Ground roll buffet effect enabled only while on ground
    Given an FFB profile with ground-roll buffet enabled
    When the aircraft transitions from airborne to on-ground
    Then the buffet effect SHALL activate only while the on-ground flag is set and deactivate on lift-off  @AC-258.6
  Scenario: FFB profile configurable in same profile file as axis profiles
    Given a profile YAML/TOML file containing both an axis section and an FFB section
    When the profile is loaded by the profile manager
    Then both the axis configuration and the FFB configuration SHALL be applied from the same profile file
