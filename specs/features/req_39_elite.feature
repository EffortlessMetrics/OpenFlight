@REQ-39
Feature: Elite: Dangerous Status.json file-watcher adapter

  @AC-39.1
  Scenario: Adapter reads Status.json and publishes BusSnapshot
    Given an EliteAdapter with a journal directory containing a Status.json file
    And the Status.json has GEAR_DOWN flag set
    When poll_once is called
    Then the adapter SHALL emit a BusSnapshot with sim "EliteDangerous"
    And config.gear SHALL be all-down

  @AC-39.1
  Scenario: Adapter returns None when Status.json is absent
    Given an EliteAdapter with an empty journal directory
    When poll_once is called
    Then no BusSnapshot SHALL be returned

  @AC-39.2
  Scenario: Identical flags on successive polls are deduplicated
    Given an EliteAdapter that has already published a snapshot with flags 0
    When poll_once is called again with identical Status.json content
    Then no BusSnapshot SHALL be returned (change detection suppresses duplicate)

  @AC-39.2
  Scenario: Changed flags on successive polls produce a new snapshot
    Given an EliteAdapter that published a snapshot with GEAR_DOWN cleared
    When Status.json is updated to set GEAR_DOWN
    Then poll_once SHALL return a new snapshot with gear all-down

  @AC-39.3
  Scenario: Lights-on flag maps to nav and landing lights
    Given a Status.json with LIGHTS_ON flag set
    When the adapter converts the status
    Then config.lights.nav SHALL be true
    And config.lights.landing SHALL be true
    And config.lights.beacon SHALL be false

  @AC-39.3
  Scenario: Fuel quantities populate config.fuel map
    Given a Status.json with FuelMain 16.0 and FuelReservoir 4.0
    When the adapter converts the status
    Then config.fuel["main"] SHALL equal 80 percent
    And the snapshot SHALL contain the "main" fuel entry

  @AC-39.4
  Scenario: Docked flag marks snapshot as not in-flight
    Given a Status.json with DOCKED flag set
    When the adapter converts the status
    Then validity.position_valid SHALL be false
    And validity.safe_for_ffb SHALL be false

  @AC-39.4
  Scenario: In-flight (no DOCKED/LANDED) marks position as valid
    Given a Status.json with no DOCKED, LANDED, or IN_SRV flags
    When the adapter converts the status
    Then validity.position_valid SHALL be true

  @AC-39.5
  Scenario: Journal LoadGame event updates current ship name
    Given an EliteAdapter
    When a LoadGame journal event for ship "SideWinder" is processed
    Then subsequent snapshots SHALL use "SideWinder" as the aircraft identifier

  @AC-39.5
  Scenario: Protocol parses Status.json flags and fuel correctly
    Given a raw Status.json payload with Flags and Fuel fields
    When the payload is deserialised into StatusJson
    Then flags SHALL match the raw bitmask
    And fuel_main and fuel_reservoir SHALL be populated

  @AC-39.6
  Scenario: Adapter lifecycle starts connected and stops cleanly
    Given an EliteAdapter
    When start is called
    Then adapter state SHALL be Connected
    When stop is called
    Then adapter state SHALL be Disconnected

  @AC-39.7
  Scenario: JournalReader finds and tails the latest journal file
    Given a journal directory containing a Journal.*.log file with a LoadGame event
    When read_new_events is called
    Then the reader SHALL return the LoadGame event
    And a second call with no new data SHALL return an empty list

  @AC-39.7
  Scenario: JournalReader tails only new events since last call
    Given a JournalReader that has already consumed one event
    When a new FsdJump event is appended to the journal file
    Then read_new_events SHALL return only the FsdJump event

  @AC-39.8
  Scenario: FsdJump event updates current star system
    Given an EliteAdapter
    When a FsdJump event for "Colonia" is applied
    Then current_system SHALL equal "Colonia"
    And docked_station SHALL be None

  @AC-39.8
  Scenario: Docked event sets station and system
    Given an EliteAdapter
    When a Docked event for "Jameson Memorial" in "Shinrarta Dezhra" is applied
    Then docked_station SHALL equal "Jameson Memorial"
    And current_system SHALL equal "Shinrarta Dezhra"

  @AC-39.8
  Scenario: Current system appears in BusSnapshot navigation waypoint
    Given an EliteAdapter that has processed a FsdJump to "Sagittarius A*"
    When convert_status is called
    Then navigation.active_waypoint SHALL equal "Sagittarius A*"
