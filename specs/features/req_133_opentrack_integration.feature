@REQ-133 @product
Feature: OpenTrack head tracking integration  @AC-133.1
  Scenario: OpenTrack UDP output parsed correctly
    Given an OpenTrack UDP packet containing yaw=10.5, pitch=-5.2, roll=2.0 degrees
    When the packet is parsed by the OpenTrack adapter
    Then the parsed yaw SHALL be 10.5 degrees, pitch -5.2 degrees, and roll 2.0 degrees  @AC-133.2
  Scenario: Head position mapped to view axis in sim
    Given a valid OpenTrack packet with yaw 45 degrees
    When the adapter maps the head position to simulator view axes
    Then the simulator view yaw axis SHALL receive the proportionally scaled value  @AC-133.3
  Scenario: Calibration deadzone at neutral head position
    Given the OpenTrack adapter is calibrated with a neutral head position
    When the received head orientation is within the configured deadzone radius
    Then the output view axis values SHALL all be zero  @AC-133.4
  Scenario: View axis range remapped to unit interval
    Given a maximum configurable head yaw of 90 degrees
    When the head yaw reaches 90 degrees
    Then the mapped view axis value SHALL be 1.0  @AC-133.5
  Scenario: Lost signal detected after N missed packets
    Given the OpenTrack adapter configured to detect signal loss after 10 missed packets
    When no UDP packet is received for 10 consecutive expected intervals
    Then the adapter SHALL set the signal-lost flag
    And the view axes SHALL be held at their last valid values  @AC-133.6
  Scenario: Reconnect after head tracker cable disconnect
    Given the OpenTrack adapter has set the signal-lost flag
    When a valid UDP packet is received from OpenTrack
    Then the signal-lost flag SHALL be cleared
    And the adapter SHALL resume normal head tracking output
