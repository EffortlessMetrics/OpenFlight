@REQ-203 @product
Feature: Devices undergo calibration to map raw hardware range to normalized output  @AC-203.1
  Scenario: Calibration wizard guides user through axis min and max detection
    Given a connected uncalibrated device
    When the calibration wizard is started via CLI or UI
    Then the wizard SHALL prompt the user to move each axis to its minimum and maximum extents  @AC-203.2
  Scenario: Calibration stored per device serial number or USB port
    Given a device has been calibrated
    When the calibration data is persisted
    Then it SHALL be keyed by device serial number or USB port identifier  @AC-203.3
  Scenario: Uncalibrated device uses conservative defaults
    Given a device with no stored calibration data
    When the device is connected and axis data is read
    Then the axis output SHALL use center 0 and range plus or minus 0.9 as conservative defaults  @AC-203.4
  Scenario: Calibration data survives service restart
    Given calibration data has been stored for a device
    When the service is restarted
    Then the previously stored calibration data SHALL be loaded and applied to the device  @AC-203.5
  Scenario: Per-axis calibration can be cleared individually
    Given a device has calibration data for multiple axes
    When the calibration for a single axis is cleared
    Then only that axis SHALL revert to conservative defaults while others retain calibration  @AC-203.6
  Scenario: CLI command flightctl calibrate runs interactive calibration
    Given a device is connected
    When the user runs flightctl calibrate
    Then the CLI SHALL launch the interactive calibration wizard for that device
