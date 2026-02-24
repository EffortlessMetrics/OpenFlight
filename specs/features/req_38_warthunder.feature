@REQ-38
Feature: War Thunder HTTP telemetry adapter

  @AC-38.1
  Scenario: Adapter publishes BusSnapshot from /indicators response
    Given a WarThunderAdapter with default configuration
    When a valid /indicators JSON response is received with airspeed 400 km/h
    Then the adapter SHALL emit a BusSnapshot with sim "WarThunder"
    And IAS SHALL be approximately 111.1 m/s (400 km/h converted)
    And the aircraft name SHALL match the airframe field

  @AC-38.1
  Scenario: Missing optional fields produce partial validity flags
    Given a WarThunderAdapter with default configuration
    When a /indicators response contains only altitude (no IAS, no attitude)
    Then position_valid SHALL be true
    And attitude_valid SHALL be false
    And safe_for_ffb SHALL be false

  @AC-38.2
  Scenario: Altitude converts from metres to feet
    Given a WarThunderAdapter with default configuration
    When a /indicators response contains altitude 2000 m
    Then environment.altitude SHALL be approximately 6562 feet

  @AC-38.2
  Scenario: Gear ratio maps to GearState correctly
    Given a WarThunderAdapter with default configuration
    When a /indicators response contains gear 0.0 (retracted)
    Then config.gear SHALL be all-up
    When a /indicators response contains gear 1.0 (deployed)
    Then config.gear SHALL be all-down

  @AC-38.2
  Scenario: Flaps ratio maps to Percentage correctly
    Given a WarThunderAdapter with default configuration
    When a /indicators response contains flaps 0.5
    Then config.flaps SHALL equal 50 percent

  @AC-38.3
  Scenario: Protocol struct deserialises War Thunder JSON field names
    Given a raw /indicators JSON payload with fields "IAS km/h", "gLoad", "vertSpeed"
    When the payload is deserialised into WtIndicators
    Then ias_kmh, g_load, and vert_speed SHALL be populated correctly

  @AC-38.3
  Scenario: Valid flag false causes poll_once to return None
    Given a WarThunderAdapter that receives "valid": false
    When poll_once is called
    Then no BusSnapshot SHALL be published

  @AC-38.4
  Scenario: Adapter lifecycle starts connected and stops disconnected
    Given a WarThunderAdapter
    When start is called
    Then adapter state SHALL be Connected
    When stop is called
    Then adapter state SHALL be Disconnected

  @AC-38.4
  Scenario: Adapter reports timeout when no packets have been received
    Given a newly created WarThunderAdapter that has not been polled
    When is_connection_timeout is checked
    Then it SHALL return true
