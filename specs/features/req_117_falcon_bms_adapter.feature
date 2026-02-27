@REQ-117 @product
Feature: Falcon BMS adapter

  @AC-117.1
  Scenario: Parse F-16C cockpit state from shared memory
    Given a Falcon BMS shared memory region with valid F-16C cockpit data
    When the adapter reads the shared memory
    Then the cockpit state SHALL be parsed without error
    And all cockpit fields SHALL reflect the values written to shared memory

  @AC-117.2
  Scenario: Detect F-16A vs F-16C from aircraft ID
    Given a Falcon BMS shared memory region
    When the aircraft ID field indicates F-16C
    Then the detected variant SHALL be F16C
    When the aircraft ID field indicates F-16A
    Then the detected variant SHALL be F16A

  @AC-117.3
  Scenario: Gear down state correctly decoded from bitmask
    Given a Falcon BMS shared memory region
    When the gear bitmask has all three gear-down bits set
    Then the gear state SHALL report all three gears as down
    When the gear bitmask has all gear-down bits clear
    Then the gear state SHALL report all three gears as up

  @AC-117.4
  Scenario: Flap state correctly decoded
    Given a Falcon BMS shared memory region
    When the flap position field is set to fully extended
    Then the decoded flap ratio SHALL be 1.0
    When the flap position field is set to retracted
    Then the decoded flap ratio SHALL be 0.0

  @AC-117.5
  Scenario: Airspeed in knots from SimConnect interface
    Given a Falcon BMS adapter connected to the SimConnect interface
    When the indicated airspeed data is received
    Then the airspeed value SHALL be expressed in knots
    And the value SHALL be within the range [0.0, 1500.0]

  @AC-117.6
  Scenario: Heading in degrees 0-359
    Given a Falcon BMS adapter receiving heading data
    When the magnetic heading is read from shared memory
    Then the heading value SHALL be in the range [0, 359] degrees
    And a heading of 360 SHALL be normalised to 0

  @AC-117.7
  Scenario: Adapter reports error on short buffer
    Given a Falcon BMS adapter
    When the shared memory buffer is shorter than the minimum required size
    Then the adapter SHALL return a ShortBuffer error
    And no cockpit state SHALL be emitted

  @AC-117.8
  Scenario: Aircraft state round-trip through profile
    Given a Falcon BMS adapter wired to the profile pipeline
    When a complete cockpit state is parsed from shared memory
    Then the state SHALL be forwarded to the active profile without loss
    And the profile SHALL apply any configured axis mappings
