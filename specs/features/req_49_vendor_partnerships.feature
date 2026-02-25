# REQ-49: Vendor Partnerships — VPforce, WinWing, Moza

Feature: Vendor device support for VPforce Rhino, WinWing HOTAS, and Moza FFB Base

  Background:
    Given the Flight Hub service is running
    And the relevant vendor device is connected via USB

  # ─── VPforce Rhino FFB ──────────────────────────────────────────────────────

  Scenario: Parse a centred Rhino V2 input report
    Given a 20-byte VPforce Rhino HID input report with all axes at mid-scale
    When the report is parsed
    Then roll, pitch, and throttle axes should normalise to 0.0 or 0.5 as appropriate
    And no parse error should be returned

  Scenario: Parse a full-deflection Rhino input report
    Given a 20-byte Rhino report with X axis at maximum raw value
    When the report is parsed
    Then the roll axis should normalise to +1.0

  Scenario: Detect correct VID and PID for Rhino V2
    When the HID subsystem enumerates devices
    Then a device with VID 0x0483 and PID 0xA1C0 should be identified as a VPforce Rhino V2
    And it should use the Rhino input parser

  Scenario: Apply Spring force-feedback effect
    Given a connected VPforce Rhino device
    When a Spring FFB effect is requested with coefficient 0.5
    Then the serialised output report should have report ID 0x10
    And the spring coefficient bytes should reflect 0.5

  Scenario: Stop all FFB effects
    Given active FFB effects on a Rhino
    When a StopAll command is issued
    Then the output report should contain the stop-all byte sequence

  Scenario: Rhino health monitor flags unhealthy after three consecutive failures
    Given a RhinoHealthMonitor with no prior failures
    When three consecutive report read failures are recorded
    Then the monitor should report the device as offline
    And the ghost rate should remain 0.0

  # ─── WinWing HOTAS ──────────────────────────────────────────────────────────

  Scenario: Parse a centred WinWing Orion2 Throttle report
    Given a 24-byte WinWing Orion2 Throttle HID report with all axes at mid-scale
    When the report is parsed
    Then the left and right throttle axes should normalise to approximately 0.5
    And no buttons should be active

  Scenario: Parse a centred WinWing Orion2 Stick report
    Given a 12-byte WinWing Orion2 Stick HID report with all axes at mid-scale
    When the report is parsed
    Then roll, pitch, twist, and throttle axes should all normalise to 0.0

  Scenario: Parse a WinWing TFRP Rudder pedal report
    Given an 8-byte WinWing TFRP HID report with toe brakes at neutral
    When the report is parsed
    Then left and right toe brakes should normalise to 0.0
    And the rudder axis should normalise to 0.0

  Scenario: Detect correct VID for all WinWing devices
    When the HID subsystem enumerates devices
    Then devices with VID 0x4098 should be identified as WinWing peripherals
    And PID 0xBE62 should map to the Orion2 Throttle parser
    And PID 0xBE63 should map to the Orion2 Stick parser
    And PID 0xBE64 should map to the TFRP rudder parser

  Scenario: WinWing throttle full forward
    Given a 24-byte Orion2 Throttle report with both axes at maximum
    When the report is parsed
    Then both throttle axes should normalise to 1.0

  Scenario: WinWing health monitor resets failures on success
    Given a WinWingHealthMonitor with two recorded failures
    When a successful report is recorded
    Then the monitor should report the device as connected

  # ─── Moza AB9 FFB Base ──────────────────────────────────────────────────────

  Scenario: Parse a centred Moza AB9 input report
    Given a 16-byte Moza AB9 HID input report with all axes at mid-scale
    When the report is parsed
    Then roll and pitch axes should normalise to 0.0
    And no buttons should be active

  Scenario: Send a centering torque command
    Given a Moza AB9 TorqueCommand with x=0.0 and y=0.0
    When the command is serialised to a report
    Then the output report should have length 5
    And the torque bytes should be zero

  Scenario: Torque command clamping prevents over-range values
    Given a TorqueCommand with x=2.0 and y=-2.0
    When is_safe is called
    Then it should return false

  Scenario: Moza health monitor torque fault makes device unhealthy
    Given a MozaHealthMonitor with no faults
    When a torque fault is set
    Then is_healthy should return false
    And clearing the fault should restore healthy status
