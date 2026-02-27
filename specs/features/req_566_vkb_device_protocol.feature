Feature: VKB Device Protocol Support
  As a flight simulation enthusiast
  I want the service to support VKB-specific device protocol features
  So that VKB HOTAS devices work correctly with full resolution

  Background:
    Given the OpenFlight service is running
    And a VKB device with VID 0x231D is connected

  Scenario: VKB devices are identified by 0x231D VID
    When the HID enumerator lists connected devices
    Then the device with VID 0x231D is tagged as a VKB device in the device registry

  Scenario: VKB 16-bit axis resolution is correctly normalized
    Given the VKB device reports a raw axis value of 32768 on a 16-bit range
    When the axis pipeline normalizes the value
    Then the normalized output is 0.5

  Scenario: VKB device configuration mode is documented in manifest quirks
    Given the compatibility manifest for the VKB Gladiator NXT EVO
    When the manifest is inspected
    Then the quirks section documents how to enter VKB configuration mode

  Scenario: VKB compatibility manifests cover all major device families
    When the manifest library is queried for VKB devices
    Then manifests exist for the Gladiator, MCG, and MCGU device families
