@REQ-181 @product
Feature: Turtle Beach VelocityOne devices operate as first-class controllers

  @AC-181.1
  Scenario: VelocityOne Flightdeck yoke detected and axes mapped correctly
    Given a Turtle Beach VelocityOne Flightdeck yoke is connected
    When the device is enumerated by the HID subsystem
    Then all yoke axes SHALL be detected and mapped to their correct flight control functions

  @AC-181.2
  Scenario: VelocityOne Rudder pedal axes normalized
    Given a Turtle Beach VelocityOne Rudder pedal unit is connected
    When the rudder pedals are moved across their full travel
    Then the pedal axes SHALL be normalized to the range [-1.0, 1.0]

  @AC-181.3
  Scenario: VelocityOne Stick detected as joystick input device
    Given a Turtle Beach VelocityOne Stick is connected
    When the HID subsystem enumerates the device
    Then it SHALL be detected and registered as a joystick input device with all axes available

  @AC-181.4
  Scenario: Differential braking axes exposed as separate left and right brake axes
    Given a Turtle Beach VelocityOne Rudder with differential toe brakes is connected
    When the left and right toe brakes are applied independently
    Then left_brake and right_brake SHALL be reported as separate, independent axes

  @AC-181.5
  Scenario: Toe-brake axes configurable via profile
    Given a profile that overrides the toe-brake axis sensitivity for a VelocityOne Rudder
    When the profile is loaded
    Then the toe-brake axes SHALL use the configured sensitivity settings

  @AC-181.6
  Scenario: All VelocityOne devices work together simultaneously
    Given a VelocityOne Flightdeck, Rudder, and Stick are all connected at the same time
    When inputs are generated on each device
    Then all devices SHALL be processed simultaneously without interference or axis conflicts
