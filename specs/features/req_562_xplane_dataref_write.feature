Feature: X-Plane DataRef Write Support
  As a flight simulation enthusiast
  I want the X-Plane adapter to write DataRef values
  So that I can drive sim variables from hardware inputs

  Background:
    Given the OpenFlight service is running
    And the X-Plane adapter is connected via UDP extplane protocol

  Scenario: Adapter writes float DataRef values via UDP extplane protocol
    Given a write-only DataRef "sim/cockpit2/radios/actuators/com1_frequency_hz_833" is configured
    When the rules engine emits a DataRef write for value 122800.0
    Then the adapter sends an extplane SET command for that DataRef over UDP

  Scenario: DataRef writes are rate-limited to avoid flooding X-Plane
    Given the DataRef write rate limit is 20 Hz
    When the rules engine emits 100 writes in one second for the same DataRef
    Then at most 20 SET commands are sent to X-Plane for that DataRef in that second

  Scenario: Write-only DataRefs are listed in adapter config
    Given the adapter config file defines a "write_datarefs" section
    When the adapter initialises
    Then only DataRefs listed in that section are eligible for write operations

  Scenario: Write failures are logged with DataRef name and value
    Given the UDP socket to X-Plane is unavailable
    When the adapter attempts to write DataRef "sim/cockpit2/switches/landing_lights_on" with value 1.0
    Then a warning log entry includes the DataRef name and the attempted value
