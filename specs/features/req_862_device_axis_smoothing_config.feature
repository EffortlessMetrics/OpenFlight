Feature: Device Axis Smoothing Config
  As a flight simulation enthusiast
  I want device axis smoothing config
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Per-device smoothing presets are available for common device types
    Given the system is configured for device axis smoothing config
    When the feature is exercised
    Then per-device smoothing presets are available for common device types

  Scenario: Custom smoothing parameters can be configured per axis
    Given the system is configured for device axis smoothing config
    When the feature is exercised
    Then custom smoothing parameters can be configured per axis

  Scenario: Smoothing algorithm selection includes moving average and Kalman filter
    Given the system is configured for device axis smoothing config
    When the feature is exercised
    Then smoothing algorithm selection includes moving average and Kalman filter

  Scenario: Smoothing configuration changes take effect within one processing cycle
    Given the system is configured for device axis smoothing config
    When the feature is exercised
    Then smoothing configuration changes take effect within one processing cycle
