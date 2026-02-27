@REQ-178 @product
Feature: Simucube 2 DD base delivers accurate force feedback

  @AC-178.1
  Scenario: Simucube 2 Sport, Pro, and Ultimate detected by USB VID/PID
    Given one of a Simucube 2 Sport, Pro, or Ultimate base is connected via USB
    When the HID subsystem enumerates connected devices
    Then the device SHALL be identified by its USB VID/PID and reported as a supported Simucube 2 base

  @AC-178.2
  Scenario: Force feedback delivered via Granity/IONI protocol
    Given a Simucube 2 base is connected and FFB is enabled in the profile
    When a force feedback effect is requested
    Then the effect SHALL be delivered to the base via the Granity/IONI protocol

  @AC-178.3
  Scenario: Torque output clamped to rated limits per model
    Given a Simucube 2 base with model-specific torque limits configured
    When a force feedback command exceeds the rated torque limit for the connected model
    Then the output torque SHALL be clamped to the rated limit for that model

  @AC-178.4
  Scenario: Encoder resolution preserved in position reports
    Given a Simucube 2 base with a 22-bit or 24-bit encoder
    When the steering wheel is rotated
    Then the position SHALL be reported at the full encoder resolution without precision loss

  @AC-178.5
  Scenario: Spring, damper, friction, and constant-force effects supported
    Given a Simucube 2 base with FFB enabled
    When spring, damper, friction, and constant-force effects are each applied in turn
    Then each effect type SHALL be transmitted and produce the corresponding force on the wheel

  @AC-178.6
  Scenario: Emergency stop cuts torque immediately
    Given a Simucube 2 base is actively producing force feedback
    When an emergency stop signal is issued
    Then the torque output SHALL be cut to zero immediately
