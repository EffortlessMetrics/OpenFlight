@REQ-102
Feature: Virtual device and OFP-1 integration

  @AC-102.1
  Scenario: Virtual gamepad is enumerable after creation
    Given the virtual device subsystem is initialised
    When a virtual gamepad device is created
    Then the device SHALL appear as an available HID device

  @AC-102.2
  Scenario: Axis output written to a virtual device is within range
    Given a virtual device with axis controls registered
    When any axis value is written to the device
    Then the reported axis output SHALL be within the device's valid range

  @AC-102.3
  Scenario: Disconnected virtual device is handled gracefully
    Given a virtual device that has been disconnected
    When the system attempts to write to the device
    Then the error SHALL be reported without a panic or memory-safety violation

  @AC-102.4
  Scenario: OFP-1 handshake completes successfully
    Given an OFP-1 protocol emulator
    When the handshake sequence is initiated
    Then the emulator SHALL report successful capability negotiation
