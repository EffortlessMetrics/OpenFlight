@REQ-211 @product
Feature: Simucube 2 direct drive base integrates with OpenFlight axis and FFB engine  @AC-211.1
  Scenario: Simucube 2 encoder position normalized to steering axis
    Given a Simucube 2 direct drive base with a 22-bit encoder
    When encoder position data is read
    Then the position SHALL be normalized to the range [-1.0, 1.0] as a steering axis  @AC-211.2
  Scenario: FFB pipeline delivers torque commands via IONI protocol
    Given an active FFB effect targeting the Simucube 2
    When the FFB pipeline computes a torque command
    Then the torque command SHALL be delivered to the device via the IONI protocol  @AC-211.3
  Scenario: Emergency stop signal propagates immediately to service
    Given a Simucube 2 with an emergency stop button
    When the ESTOP signal is triggered
    Then the service SHALL receive the ESTOP event immediately and halt all FFB output  @AC-211.4
  Scenario: Multiple Simucube models detected by PID
    Given a Simucube 2 Sport, Pro, or Ultimate connected to the system
    When the device enumerates
    Then the correct model SHALL be identified by its USB PID and logged at startup  @AC-211.5
  Scenario: Granity firmware version read and logged at startup
    Given a Simucube 2 device connected at service start
    When the service initialises the device
    Then the Granity firmware version SHALL be read from the device and logged  @AC-211.6
  Scenario: Thermal shutdown from Simucube handled gracefully without service crash
    Given a Simucube 2 that enters thermal shutdown
    When the device reports a thermal shutdown event
    Then the service SHALL log the event, disable FFB output, and continue running without crashing
