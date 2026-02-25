Feature: REQ-44 DCS World Integration with MP-Safe Enforcement

  # AC-44.1: MP Session Detection

  @AC-44.1
  Scenario: Detect single player session via explicit session_type marker
    Given a DCS adapter with enforce_mp_integrity enabled
    When a session update is received with session_type "SP"
    Then the adapter is not in a multiplayer session
    And no features are blocked
    And the MP banner message is absent

  @AC-44.1
  Scenario: Detect multiplayer session via explicit session_type marker
    Given a DCS adapter with enforce_mp_integrity enabled
    When a session update is received with session_type "MP"
    Then the adapter is in a multiplayer session
    And "telemetry_weapons" is in the blocked features list
    And "telemetry_countermeasures" is in the blocked features list
    And "telemetry_rwr" is in the blocked features list

  @AC-44.1
  Scenario: Infer multiplayer session from server_name field
    Given a DCS adapter with enforce_mp_integrity enabled
    When a session update is received containing a "server_name" field
    Then the adapter is in a multiplayer session

  # AC-44.2: Restricted Field Filtering

  @AC-44.2
  Scenario: Restricted fields are stripped before bus publication in MP
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When filter_restricted_fields is called with data containing "weapons", "countermeasures", "rwr_contacts", and "ias"
    Then the filtered data does not contain "weapons"
    And the filtered data does not contain "countermeasures"
    And the filtered data does not contain "rwr_contacts"
    And the filtered data contains "ias"
    And the blocked list contains "weapons", "countermeasures", and "rwr_contacts"

  @AC-44.2
  Scenario: Restricted fields pass through unmodified in single player
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a single player session
    When filter_restricted_fields is called with data containing "weapons" and "ias"
    Then the filtered data contains "weapons"
    And the filtered data contains "ias"
    And the blocked list is empty

  # AC-44.3: User-Visible Blocked Feature Messages

  @AC-44.3
  Scenario: Blocked feature returns user-friendly message
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When check_feature_blocked is called with "telemetry_weapons"
    Then the message contains "multiplayer integrity"

  @AC-44.3
  Scenario: Allowed feature returns no message in MP
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When check_feature_blocked is called with "telemetry_basic"
    Then no message is returned

  # AC-44.4: MP Banner for UI Display

  @AC-44.4
  Scenario: MP session shows banner containing server name
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session named "Blue Flag 2024"
    When the MP banner is queried
    Then the banner contains "Blue Flag 2024"
    And the banner contains "Multiplayer"

  @AC-44.4
  Scenario: Single player session has no MP banner
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a single player session
    When the MP banner is queried
    Then the banner is absent

  # AC-44.5: Self-Aircraft Telemetry Always Allowed

  @AC-44.5
  Scenario: Self-aircraft kinematic data is published in MP without restriction
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When a telemetry frame with only self-aircraft fields (ias, tas, altitude, heading, pitch, bank) is converted
    Then the bus snapshot contains valid kinematic data

  # AC-44.6: Aircraft Configuration Telemetry (Gear and Flaps)

  @AC-44.6
  Scenario: Landing gear down maps to GearState all-down
    Given a DCS adapter
    When a telemetry frame with gear_down 1.0 is converted
    Then config.gear reports all gear positions as Down

  @AC-44.6
  Scenario: Landing gear up maps to GearState all-up
    Given a DCS adapter
    When a telemetry frame with gear_down 0.0 is converted
    Then config.gear reports all gear positions as Up

  @AC-44.6
  Scenario: Landing gear transitioning maps to GearState transitioning
    Given a DCS adapter
    When a telemetry frame with gear_down 0.5 is converted
    Then config.gear reports transitioning state

  @AC-44.6
  Scenario: Flaps percentage is mapped from draw argument
    Given a DCS adapter
    When a telemetry frame with flaps 30.0 is converted
    Then config.flaps value is 30.0 percent

  # AC-44.7: Lua Unit Conversions

  @AC-44.7
  Scenario: IAS and TAS are converted from m/s to knots in generated Export.lua
    Given the Export.lua generator
    When the script is generated
    Then the IAS collection code multiplies LoGetIndicatedAirSpeed() by 1.94384
    And the TAS collection code multiplies LoGetTrueAirSpeed() by 1.94384

  @AC-44.7
  Scenario: Altitude values are converted from meters to feet in generated Export.lua
    Given the Export.lua generator
    When the script is generated
    Then altitude_asl multiplies LoGetAltitudeAboveSeaLevel() by 3.28084
    And altitude_agl multiplies LoGetAltitudeAboveGroundLevel() by 3.28084

  @AC-44.7
  Scenario: Vertical speed is converted from m/s to feet per minute in generated Export.lua
    Given the Export.lua generator
    When the script is generated
    Then vertical_speed multiplies LoGetVerticalVelocity() by 196.85

  @AC-44.7
  Scenario: AoA is converted from radians to degrees in generated Export.lua
    Given the Export.lua generator
    When the script is generated
    Then aoa uses math.deg() to convert LoGetAngleOfAttack() to degrees

  @AC-44.7
  Scenario: Waypoint distance is converted from meters to NM in generated Export.lua
    Given the Export.lua generator
    When the script is generated
    Then waypoint_distance multiplies goto_point.dist by 0.000539957
