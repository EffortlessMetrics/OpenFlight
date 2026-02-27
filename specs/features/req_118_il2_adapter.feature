@REQ-118 @product
Feature: IL-2 Great Battles adapter

  @AC-118.1
  Scenario: Parse UDP telemetry frame for Spitfire
    Given an IL-2 adapter receiving a valid UDP telemetry frame
    When the frame contains Spitfire aircraft data
    Then the aircraft type SHALL be detected as Spitfire
    And all telemetry fields SHALL be populated correctly

  @AC-118.2
  Scenario: Parse UDP telemetry frame for Bf 109
    Given an IL-2 adapter receiving a valid UDP telemetry frame
    When the frame contains Bf 109 aircraft data
    Then the aircraft type SHALL be detected as Bf109
    And all telemetry fields SHALL be populated correctly

  @AC-118.3
  Scenario: Aircraft type detected from name
    Given an IL-2 adapter
    When a telemetry frame with aircraft name "Supermarine Spitfire Mk.IX" is received
    Then the adapter SHALL map the name to the Spitfire aircraft type
    When a telemetry frame with aircraft name "Messerschmitt Bf 109 G-6" is received
    Then the adapter SHALL map the name to the Bf109 aircraft type

  @AC-118.4
  Scenario: Gear state transitions decoded
    Given an IL-2 adapter
    When the gear state field transitions from retracted to extended
    Then the adapter SHALL emit a GearDown event
    When the gear state field transitions from extended to retracted
    Then the adapter SHALL emit a GearUp event

  @AC-118.5
  Scenario: Throttle clamped to [0.0, 1.0]
    Given an IL-2 adapter
    When the raw throttle value in a telemetry frame exceeds 1.0
    Then the decoded throttle SHALL be clamped to 1.0
    When the raw throttle value is below 0.0
    Then the decoded throttle SHALL be clamped to 0.0

  @AC-118.6
  Scenario: Invalid magic number rejected
    Given an IL-2 adapter
    When a UDP frame with an incorrect magic number is received
    Then the adapter SHALL discard the frame
    And a MalformedFrame error SHALL be recorded

  @AC-118.7
  Scenario: Unsupported protocol version rejected
    Given an IL-2 adapter
    When a UDP frame with a protocol version higher than the supported maximum is received
    Then the adapter SHALL discard the frame
    And an UnsupportedVersion error SHALL be recorded

  @AC-118.8
  Scenario: Telemetry round-trip through bus
    Given an IL-2 adapter connected to the flight bus
    When a valid telemetry frame is received
    Then a TelemetryUpdate event SHALL be published on the flight bus
    And the event payload SHALL match the decoded telemetry fields
