@REQ-529 @product
Feature: Axis Calibration Auto-Detection — Automatic Range Discovery  @AC-529.1
  Scenario: Auto-calibration mode monitors axis values over time
    Given auto-calibration is enabled for a joystick axis
    When the user moves the axis through its full travel
    Then the calibration subsystem SHALL record the observed minimum and maximum values  @AC-529.2
  Scenario: Detected min, max, and center are stored as calibration
    Given an auto-calibration session has observed the full axis range
    When the calibration session ends
    Then the detected min, max, and center SHALL be persisted to the calibration store  @AC-529.3
  Scenario: Auto-calibration runs for a configured duration then completes
    Given auto-calibration is configured for a 30-second duration
    When 30 seconds elapse
    Then the calibration session SHALL finalise automatically and the axis SHALL use the new values  @AC-529.4
  Scenario: Auto-calibration can be triggered via CLI
    Given flightd is running with a connected joystick
    When `flightctl calibrate --axis pitch --auto` is executed
    Then auto-calibration mode SHALL activate for the pitch axis
