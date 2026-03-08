@REQ-1053 @product @user-journey
Feature: Device hot-plug user journey
  As a pilot using OpenFlight
  I want devices to be seamlessly detected when plugged or unplugged
  So that I can reconfigure my cockpit without restarting the service

  @AC-1053.1
  Scenario: Connect a new device and it becomes active immediately
    Given the OpenFlight service is running with no devices connected
    When a USB joystick is connected
    Then the device SHALL appear in the active device list within 2 seconds
    And the device axes SHALL begin processing at 250 Hz
    And a "device_connected" event SHALL be published to IPC clients

  @AC-1053.2
  Scenario: Disconnect a device and axes return to neutral
    Given a joystick is connected and its axes are being processed
    When the joystick is unplugged
    Then all axes from that device SHALL transition to neutral values within one tick
    And the device SHALL be removed from the active device list
    And a "device_disconnected" event SHALL be published to IPC clients

  @AC-1053.3
  Scenario: Reconnect the same device restores its previous profile
    Given a joystick was connected with profile "My Stick Config" and then unplugged
    When the same joystick is plugged back in
    Then the device SHALL be re-detected within 2 seconds
    And the "My Stick Config" profile SHALL be automatically re-associated
    And axis processing SHALL resume with the restored profile settings

  @AC-1053.4
  Scenario: Connect a previously unseen device triggers profile creation
    Given the service is running with profiles for known devices
    When a brand-new USB throttle with an unrecognized VID/PID is connected
    Then the device SHALL be detected and enumerated
    And a default profile SHALL be generated for the new device
    And the user SHALL be notified that a new device was configured with defaults

  @AC-1053.5
  Scenario: Multiple devices connected simultaneously are all enumerated
    Given the OpenFlight service is running with no devices
    When a joystick, throttle, and rudder pedals are connected via a USB hub
    Then all three devices SHALL be detected and enumerated
    And each device SHALL have independent axis processing pipelines
    And the service SHALL remain stable with no duplicate device entries
