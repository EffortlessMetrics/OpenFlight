@REQ-123 @product
Feature: Aerofly FS binary telemetry and AeroWinx PSX adapter

  @AC-123.1
  Scenario: Aerofly UDP binary telemetry parses pitch, roll, and heading
    Given an Aerofly FS adapter receiving a UDP binary telemetry packet
    When the packet encodes pitch -5.0 degrees, roll 15.0 degrees, and heading 270.0 degrees
    Then the parsed pitch SHALL be -5.0 degrees
    And the parsed roll SHALL be 15.0 degrees
    And the parsed heading SHALL be 270.0 degrees

  @AC-123.2
  Scenario: Aerofly JSON telemetry parses flap ratio 0.0-1.0
    Given an Aerofly FS adapter receiving a JSON telemetry packet
    When the packet contains a "flap_ratio" field of 0.75
    Then the decoded flap ratio SHALL be 0.75
    And a "flap_ratio" value of 0.0 SHALL decode to 0.0
    And a "flap_ratio" value of 1.0 SHALL decode to 1.0

  @AC-123.3
  Scenario: Aerofly malformed packet returns error
    Given an Aerofly FS adapter
    When a UDP packet with a truncated or corrupt binary payload is received
    Then the adapter SHALL return a ParseError
    And no telemetry state SHALL be emitted for that packet
    And the adapter SHALL continue processing subsequent packets

  @AC-123.4
  Scenario: PSX line parser: valid format parses variable and value
    Given an AeroWinx PSX adapter
    When a line in the format "Qi0=12345" is received
    Then the parser SHALL extract variable identifier "Qi0"
    And the parsed value SHALL be 12345

  @AC-123.5
  Scenario: PSX accumulates multiple variables from line stream
    Given an AeroWinx PSX adapter receiving a stream of lines
    When the stream contains "Qi0=100", "Qi1=200", and "Qi2=300"
    Then the adapter SHALL store all three variable values
    And each variable SHALL be retrievable by its identifier

  @AC-123.6
  Scenario: PSX unknown variable ID is gracefully ignored
    Given an AeroWinx PSX adapter
    When a line containing an unrecognised variable identifier is received
    Then the adapter SHALL discard the line without error
    And no state SHALL be updated for the unknown variable
    And the adapter SHALL continue processing subsequent lines

  @AC-123.7
  Scenario: PSX round-trip: value set then read back
    Given an AeroWinx PSX adapter with variable "Qi5" set to 42
    When the value for "Qi5" is read back from the adapter state
    Then the returned value SHALL be 42

  @AC-123.8
  Scenario: PSX Boeing 744 FCU speed variable decoded correctly
    Given an AeroWinx PSX adapter connected to a Boeing 744 simulation
    When the FCU speed variable "Qs10" arrives with the raw value encoding 250 knots
    Then the decoded airspeed SHALL be 250 knots
