Feature: Device Firmware Compatibility
  As a flight simulation enthusiast
  I want device firmware compatibility
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Device firmware version is checked against a known compatibility list
    Given the system is configured for device firmware compatibility
    When the feature is exercised
    Then device firmware version is checked against a known compatibility list

  Scenario: Incompatible firmware triggers a warning with upgrade guidance
    Given the system is configured for device firmware compatibility
    When the feature is exercised
    Then incompatible firmware triggers a warning with upgrade guidance

  Scenario: Compatibility database is updatable without a service release
    Given the system is configured for device firmware compatibility
    When the feature is exercised
    Then compatibility database is updatable without a service release

  Scenario: Firmware version is displayed in device information output
    Given the system is configured for device firmware compatibility
    When the feature is exercised
    Then firmware version is displayed in device information output
