@REQ-168 @product
Feature: Device hot-plug

  @AC-168.1
  Scenario: New joystick detected on USB connect
    Given the HID subsystem is running
    When a joystick is connected via USB
    Then the device SHALL be detected and enumerated within 2 seconds

  @AC-168.2
  Scenario: Profile auto-assigned on connect
    Given a device profile exists for the connected device VID/PID
    When the device is connected
    Then the matching profile SHALL be automatically assigned to the device

  @AC-168.3
  Scenario: Device removed cleanly on unplug
    Given a joystick is connected and active
    When the device is unplugged
    Then the device SHALL be removed from the active device list without error

  @AC-168.4
  Scenario: Axis output goes to neutral on unplug
    Given an axis is being processed from a connected device
    When the device is unplugged
    Then the axis output SHALL transition to the neutral value immediately

  @AC-168.5
  Scenario: Re-plug re-initializes without restart
    Given a device has been unplugged
    When the same device is plugged back in
    Then the device SHALL be re-initialized and become active without requiring a service restart

  @AC-168.6
  Scenario: Same device re-associates with last profile
    Given a device was previously associated with a profile and has been unplugged
    When the same device is plugged back in
    Then the device SHALL be re-associated with its last active profile

  @AC-168.7
  Scenario: Multiple plug and unplug cycles remain stable
    Given the HID subsystem is running
    When a device is plugged and unplugged five times in succession
    Then the system SHALL remain stable with no memory leaks or errors after each cycle

  @AC-168.8
  Scenario: Hot-plug event published to IPC clients
    Given at least one IPC client is connected
    When a device is connected or disconnected
    Then a hot-plug event SHALL be published to all connected IPC clients
