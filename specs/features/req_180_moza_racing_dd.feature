@REQ-180 @product
Feature: MOZA Racing DD base integrates with OpenFlight as FFB device

  @AC-180.1
  Scenario: MOZA base detected by USB VID 0x346E
    Given a MOZA R5, R9, or R12 base is connected via USB
    When the HID subsystem enumerates connected devices
    Then the device SHALL be identified by USB VID 0x346E and reported as a supported MOZA base

  @AC-180.2
  Scenario: Steering angle, velocity, and acceleration reported at 1 kHz
    Given a MOZA base is connected and active
    When the steering wheel is in motion
    Then the steering angle, angular velocity, and angular acceleration SHALL each be reported at 1 kHz

  @AC-180.3
  Scenario: Force feedback translated from sim forces to MOZA torque commands
    Given a MOZA base with FFB enabled and a simulator producing force output
    When the simulator sends force feedback data
    Then the FFB pipeline SHALL translate the sim forces into MOZA-compatible torque commands

  @AC-180.4
  Scenario: MOZA firmware version queried and logged at startup
    Given a MOZA base is connected when the service starts
    When the device is initialised
    Then the MOZA firmware version SHALL be queried from the device and written to the service log

  @AC-180.5
  Scenario: Multiple MOZA bases supported simultaneously
    Given two or more MOZA bases are connected to the system
    When the HID subsystem enumerates devices
    Then each MOZA base SHALL be tracked independently with its own axis and FFB context

  @AC-180.6
  Scenario: MOZA thermal shutdown event handled gracefully
    Given a MOZA base is actively delivering force feedback
    When the base signals a thermal shutdown event
    Then the system SHALL acknowledge the event, halt force output to that device, and log an overheat warning
