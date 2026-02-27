Feature: Device Axis Inversion
  As a flight simulation enthusiast
  I want device axis inversion
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Per-axis inversion in profile
    Given the system is configured for device axis inversion
    When the feature is exercised
    Then device axis inversion is configurable per-axis in profile

  Scenario: Applied before curve processing
    Given the system is configured for device axis inversion
    When the feature is exercised
    Then inversion is applied before curve processing

  Scenario: Indicated in device status
    Given the system is configured for device axis inversion
    When the feature is exercised
    Then inverted axes are indicated in device status

  Scenario: Effect without reconnect
    Given the system is configured for device axis inversion
    When the feature is exercised
    Then inversion changes take effect without device reconnect
