@REQ-135 @product
Feature: OpenTrack head tracking  @AC-135.1
  Scenario: Parse 48-byte UDP packet with known values
    Given a 48-byte OpenTrack UDP packet with yaw 90.0 pitch -30.0 roll 0.0 x 0.0 y 0.0 z 0.0
    When the packet is parsed
    Then the parsed yaw SHALL be 90.0 pitch SHALL be -30.0 and roll SHALL be 0.0  @AC-135.2
  Scenario: NaN in packet returns error
    Given a 48-byte OpenTrack UDP packet containing a NaN value in the yaw field
    When the packet is parsed
    Then the result SHALL be an error indicating an invalid floating-point value  @AC-135.3
  Scenario: Too-short packet returns error
    Given an OpenTrack UDP packet that is only 32 bytes long
    When the packet is parsed
    Then the result SHALL be an error indicating an insufficient packet length  @AC-135.4
  Scenario: Yaw 0 degrees normalizes to 0.5
    Given a parsed OpenTrack frame with yaw 0.0 degrees
    When the yaw value is normalized to a 0.0–1.0 axis range
    Then the normalized yaw axis value SHALL be 0.5  @AC-135.5
  Scenario: Pitch 0 degrees normalizes to 0.5
    Given a parsed OpenTrack frame with pitch 0.0 degrees
    When the pitch value is normalized to a 0.0–1.0 axis range
    Then the normalized pitch axis value SHALL be 0.5  @AC-135.6
  Scenario: Yaw minus 180 degrees maps to 0.0 axis value
    Given a parsed OpenTrack frame with yaw -180.0 degrees
    When the yaw value is normalized to a 0.0–1.0 axis range
    Then the normalized yaw axis value SHALL be 0.0  @AC-135.7
  Scenario: Yaw plus 180 degrees maps to 1.0 axis value
    Given a parsed OpenTrack frame with yaw 180.0 degrees
    When the yaw value is normalized to a 0.0–1.0 axis range
    Then the normalized yaw axis value SHALL be 1.0  @AC-135.8
  Scenario: Deadzone at neutral head position suppresses small movements
    Given a deadzone of 5 percent configured for the yaw axis
    When the parsed OpenTrack yaw is 2.0 degrees from neutral
    Then the output yaw axis SHALL be zero
