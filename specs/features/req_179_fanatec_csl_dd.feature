@REQ-179 @product
Feature: Fanatec direct-drive base integrates with OpenFlight FFB pipeline

  @AC-179.1
  Scenario: Fanatec CSL DD detected by USB VID 0x0EB7
    Given a Fanatec CSL DD base is connected via USB
    When the HID subsystem enumerates connected devices
    Then the device SHALL be identified by USB VID 0x0EB7 and reported as a supported Fanatec CSL DD base

  @AC-179.2
  Scenario: Wheel angle reported in high resolution with configurable steering lock
    Given a Fanatec CSL DD with a steering lock angle configured in the profile
    When the steering wheel is rotated across its full range
    Then the wheel angle SHALL be reported in high resolution using the configured steering lock as the normalization range

  @AC-179.3
  Scenario: FFB pipeline translates flight sim forces to wheel torques
    Given a Fanatec CSL DD with FFB enabled and a flight simulator producing force data
    When the simulator sends force feedback output
    Then the FFB pipeline SHALL translate the sim forces into appropriate wheel torque commands

  @AC-179.4
  Scenario: Fanatec-specific HID protocol extensions handled gracefully
    Given a Fanatec CSL DD that uses extended HID report descriptors
    When the driver parses device reports
    Then any Fanatec-specific HID protocol extensions SHALL be parsed without errors or dropped data

  @AC-179.5
  Scenario: Compatible with PC mode and console mode configurations
    Given a Fanatec CSL DD configured in either PC mode or console mode
    When the device is enumerated
    Then axis and button inputs SHALL be accessible under both mode configurations

  @AC-179.6
  Scenario: Loss of wheel signal triggers safe-mode on associated axes
    Given a Fanatec CSL DD is active and providing wheel input
    When the wheel signal is lost unexpectedly
    Then the system SHALL engage safe-mode on all axes associated with the wheel and log the event
