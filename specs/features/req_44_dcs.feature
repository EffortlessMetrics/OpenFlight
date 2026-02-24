Feature: REQ-44 DCS World Integration with MP-Safe Enforcement

  # AC-44.1: MP Session Detection

  Scenario: Detect single player session via explicit session_type marker
    Given a DCS adapter with enforce_mp_integrity enabled
    When a session update is received with session_type "SP"
    Then the adapter is not in a multiplayer session
    And no features are blocked
    And the MP banner message is absent

  Scenario: Detect multiplayer session via explicit session_type marker
    Given a DCS adapter with enforce_mp_integrity enabled
    When a session update is received with session_type "MP"
    Then the adapter is in a multiplayer session
    And "telemetry_weapons" is in the blocked features list
    And "telemetry_countermeasures" is in the blocked features list
    And "telemetry_rwr" is in the blocked features list

  Scenario: Infer multiplayer session from server_name field
    Given a DCS adapter with enforce_mp_integrity enabled
    When a session update is received containing a "server_name" field
    Then the adapter is in a multiplayer session

  # AC-44.2: Restricted Field Filtering

  Scenario: Restricted fields are stripped before bus publication in MP
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When filter_restricted_fields is called with data containing "weapons", "countermeasures", "rwr_contacts", and "ias"
    Then the filtered data does not contain "weapons"
    And the filtered data does not contain "countermeasures"
    And the filtered data does not contain "rwr_contacts"
    And the filtered data contains "ias"
    And the blocked list contains "weapons", "countermeasures", and "rwr_contacts"

  Scenario: Restricted fields pass through unmodified in single player
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a single player session
    When filter_restricted_fields is called with data containing "weapons" and "ias"
    Then the filtered data contains "weapons"
    And the filtered data contains "ias"
    And the blocked list is empty

  # AC-44.3: User-Visible Blocked Feature Messages

  Scenario: Blocked feature returns user-friendly message
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When check_feature_blocked is called with "telemetry_weapons"
    Then the message contains "multiplayer integrity"

  Scenario: Allowed feature returns no message in MP
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When check_feature_blocked is called with "telemetry_basic"
    Then no message is returned

  # AC-44.4: MP Banner for UI Display

  Scenario: MP session shows banner containing server name
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session named "Blue Flag 2024"
    When the MP banner is queried
    Then the banner contains "Blue Flag 2024"
    And the banner contains "Multiplayer"

  Scenario: Single player session has no MP banner
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a single player session
    When the MP banner is queried
    Then the banner is absent

  # AC-44.5: Self-Aircraft Telemetry Always Allowed

  Scenario: Self-aircraft kinematic data is published in MP without restriction
    Given a DCS adapter with enforce_mp_integrity enabled
    And the adapter is in a multiplayer session
    When a telemetry frame with only self-aircraft fields (ias, tas, altitude, heading, pitch, bank) is converted
    Then the bus snapshot contains valid kinematic data
