@REQ-119 @product
Feature: Aerofly FS adapter

  @AC-119.1
  Scenario: Parse JSON telemetry from Aerofly FS 4
    Given an Aerofly FS adapter receiving a UDP JSON telemetry packet
    When the packet contains valid Aerofly FS 4 telemetry
    Then the telemetry SHALL be parsed without error
    And all fields SHALL reflect the values in the JSON payload

  @AC-119.2
  Scenario: Aircraft type detected from JSON
    Given an Aerofly FS adapter
    When the JSON payload contains aircraft name "Airbus A320"
    Then the detected aircraft type SHALL be AirbusA320
    When the JSON payload contains aircraft name "Boeing 737-500"
    Then the detected aircraft type SHALL be Boeing737

  @AC-119.3
  Scenario: Pitch, roll, and heading extracted correctly
    Given an Aerofly FS adapter
    When a JSON telemetry packet specifies pitch -5.0 degrees, roll 15.0 degrees, and heading 270.0 degrees
    Then the parsed pitch SHALL be -5.0 degrees
    And the parsed roll SHALL be 15.0 degrees
    And the parsed heading SHALL be 270.0 degrees

  @AC-119.4
  Scenario: Gear state from JSON boolean
    Given an Aerofly FS adapter
    When the JSON payload has "gear_down" set to true
    Then the decoded gear state SHALL be down
    When the JSON payload has "gear_down" set to false
    Then the decoded gear state SHALL be up

  @AC-119.5
  Scenario: Flap ratio 0.0-1.0 from float
    Given an Aerofly FS adapter
    When the JSON payload contains "flap_ratio" of 0.5
    Then the decoded flap ratio SHALL be 0.5
    And values outside [0.0, 1.0] SHALL be clamped to the valid range

  @AC-119.6
  Scenario: Adapter connects on UDP port 49002
    Given an Aerofly FS adapter configured with default settings
    When the adapter is started
    Then it SHALL bind to UDP port 49002
    And it SHALL begin receiving telemetry on that port

  @AC-119.7
  Scenario: Malformed JSON gracefully rejected
    Given an Aerofly FS adapter
    When a UDP packet containing malformed JSON is received
    Then the adapter SHALL discard the packet
    And a ParseError SHALL be recorded
    And the adapter SHALL continue processing subsequent packets
