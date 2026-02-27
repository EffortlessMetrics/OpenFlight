Feature: Axis Hysteresis Width Config
  As a flight simulation enthusiast
  I want axis hysteresis width config
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Configurable width per-axis in profile
    Given the system is configured for axis hysteresis width config
    When the feature is exercised
    Then axis hysteresis width is configurable per-axis in the profile

  Scenario: Prevent jitter without noticeable lag
    Given the system is configured for axis hysteresis width config
    When the feature is exercised
    Then hysteresis prevents jitter without adding noticeable input lag

  Scenario: Validate width against resolution bounds
    Given the system is configured for axis hysteresis width config
    When the feature is exercised
    Then width value is validated against axis resolution bounds at load time

  Scenario: Zero width disables the filter
    Given the system is configured for axis hysteresis width config
    When the feature is exercised
    Then zero hysteresis width disables the filter for that axis
