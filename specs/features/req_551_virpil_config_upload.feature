@REQ-551 @product
Feature: VIRPIL Config Upload — Service should support uploading axis configuration to VIRPIL devices

  @AC-551.1
  Scenario: VIRPIL HID config reports are understood and documented
    Given the VIRPIL protocol documentation in the compatibility manifest
    When the manifest is validated
    Then it SHALL contain HID config report format descriptions for VIRPIL configuration commands

  @AC-551.2
  Scenario: Axis calibration can be written back to device
    Given a connected VIRPIL device and a calibration profile
    When the upload-calibration command is issued
    Then the service SHALL write the calibration data to the device via HID config report

  @AC-551.3
  Scenario: Config upload requires explicit user confirmation
    Given a VIRPIL config upload is requested
    When the service prepares to write config to the device
    Then it SHALL prompt the user for confirmation before proceeding

  @AC-551.4
  Scenario: Failed uploads are retried once
    Given a VIRPIL config upload that fails on the first attempt
    When the upload error is detected
    Then the service SHALL retry the upload exactly once before reporting failure
