@REQ-512 @product
Feature: Device Firmware Version Tracking

  @AC-512.1
  Scenario: HID device firmware version is read from USB descriptor
    Given a HID device with a firmware version in its USB descriptor
    When the device is enumerated by the service
    Then the firmware version SHALL be read and stored in the device info record

  @AC-512.2 @AC-512.3
  Scenario: Outdated firmware triggers a warning in the service log
    Given the compat manifest specifies a minimum firmware version for a device
    When a device with an older firmware version connects
    Then a warning SHALL be logged identifying the device and its firmware version

  @AC-512.4
  Scenario: Firmware version is included in diagnostic bundle
    Given one or more HID devices are connected
    When a diagnostic bundle is generated
    Then the bundle SHALL contain the firmware version for each connected device
