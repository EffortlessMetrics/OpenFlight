Feature: Device Polling Rate Detection
  As a flight simulation enthusiast
  I want device polling rate detection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Measure actual polling rate
    Given the system is configured for device polling rate detection
    When the feature is exercised
    Then service measures actual device polling rate over a sample window

  Scenario: Compare against expected rate
    Given the system is configured for device polling rate detection
    When the feature is exercised
    Then detected rate is compared against expected rate from device descriptor

  Scenario: Warn on significant deviation
    Given the system is configured for device polling rate detection
    When the feature is exercised
    Then significant deviation triggers a warning

  Scenario: Expose in device status
    Given the system is configured for device polling rate detection
    When the feature is exercised
    Then polling rate is exposed in device status
