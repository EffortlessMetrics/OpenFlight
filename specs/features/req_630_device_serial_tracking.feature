Feature: Device Serial Number Tracking
  As a flight simulation enthusiast
  I want the service to track device serial numbers
  So that my devices are identified consistently regardless of which USB port they use

  Background:
    Given the OpenFlight service is running

  Scenario: Serial number is read from HID string descriptor if available
    Given a HID device with a serial number in its string descriptor
    When the device is connected
    Then the service reads the serial number from the HID string descriptor

  Scenario: Serial number is used as stable device identity across USB ports
    Given a device with a known serial number
    When the device is reconnected to a different USB port
    Then the service identifies the device by its serial number rather than its port

  Scenario: Calibration store is keyed by serial number
    Given a device with a serial number has calibration data stored
    When the device is reconnected
    Then the previously stored calibration data is loaded using the serial number as key

  Scenario: Serial number is shown in flightctl devices output
    When the operator runs flightctl devices
    Then the serial number is displayed for each device that provides one
