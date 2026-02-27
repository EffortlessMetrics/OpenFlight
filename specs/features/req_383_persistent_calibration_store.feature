@REQ-383 @product
Feature: Persistent Axis Calibration Store  @AC-383.1
  Scenario: Calibration data is written to the platform config directory
    Given an axis calibration operation has been completed for a device
    When the calibration is saved
    Then min, max, and center values SHALL be written to the platform-appropriate config directory  @AC-383.2
  Scenario: Saved calibration is loaded and applied automatically on startup
    Given previously saved calibration data exists in the config directory
    When the service starts with the calibrated device connected
    Then the saved calibration SHALL be loaded and applied to the corresponding axes  @AC-383.3
  Scenario: Calibration store uses TOML format keyed by device VID and PID
    Given a saved calibration file on disk
    When the file format is inspected
    Then it SHALL be TOML with device VID/PID as the top-level key  @AC-383.4
  Scenario: Missing calibration file is handled gracefully with defaults
    Given no calibration file exists for a connected device
    When the service starts with that device connected
    Then default calibration values SHALL be applied without error  @AC-383.5
  Scenario: flightctl calibrate reset removes saved calibration for a device
    Given saved calibration exists for a specific device
    When the user runs flightctl calibrate reset for that device
    Then the calibration entry for that device SHALL be removed from the store  @AC-383.6
  Scenario: Calibration data survives a service restart without re-calibrating
    Given calibration has been saved and the service is restarted
    When axes begin processing input after restart
    Then the previously saved calibration SHALL be applied without manual re-calibration
