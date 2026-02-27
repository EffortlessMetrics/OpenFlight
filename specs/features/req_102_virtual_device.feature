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

  @AC-102.5
  Scenario: Button state written to a virtual device is correctly reported
    Given a virtual device with button controls registered
    When a button state is written to the device
    Then the reported button output SHALL reflect the written state

  @AC-102.6
  Scenario: OFP-1 emulator handles an emergency stop without panicking
    Given an initialised OFP-1 emulator
    When an emergency stop command is issued
    Then the emulator SHALL enter a safe stopped state without panicking

  @AC-102.7
  Scenario: Duplicate virtual device registration is rejected
    Given a device manager with a virtual device already registered
    When a second device with the same identifier is registered
    Then the registration SHALL be rejected with an appropriate error
