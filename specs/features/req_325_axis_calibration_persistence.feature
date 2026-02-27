@REQ-325 @product
Feature: Axis Calibration Persistence  @AC-325.1
  Scenario: Calibration data is stored in a per-device JSON file
    Given a device has been calibrated via flightctl calibrate
    When calibration completes successfully
    Then the service SHALL persist calibration data to a JSON file in the calibration store  @AC-325.2
  Scenario: Calibration file is named by VID/PID
    Given a calibrated device with VID 046D and PID C24F
    When the calibration store is inspected
    Then the service SHALL write the file as calibration_046D_C24F.json  @AC-325.3
  Scenario: Calibration is applied automatically on device connect
    Given a calibration file exists for a device
    When that device is connected
    Then the service SHALL load and apply the stored calibration without user intervention  @AC-325.4
  Scenario: Manual calibration override is supported per-profile
    Given a profile that includes a per-profile axis calibration override
    When that profile is active
    Then the service SHALL apply the profile calibration override instead of the stored device calibration  @AC-325.5
  Scenario: Expired calibration generates a reminder log
    Given a calibration file whose timestamp is more than 30 days old
    When the device connects and the calibration is loaded
    Then the service SHALL emit a reminder log entry stating the calibration has expired  @AC-325.6
  Scenario: Calibration store path is configurable
    Given the service configuration specifies a custom calibration store path
    When the service starts
    Then the service SHALL read and write calibration files at the configured path, defaulting to ~/.config/openflight/
