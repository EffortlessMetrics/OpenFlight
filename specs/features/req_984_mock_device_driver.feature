Feature: Mock Device Driver
  As a flight simulation enthusiast
  I want mock device driver
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Virtual devices simulate HID input for testing without physical hardware
    Given the system is configured for mock device driver
    When the feature is exercised
    Then virtual devices simulate HID input for testing without physical hardware

  Scenario: Mock devices support configurable axis count, button count, and FFB capabilities
    Given the system is configured for mock device driver
    When the feature is exercised
    Then mock devices support configurable axis count, button count, and FFB capabilities

  Scenario: Scripted input sequences can be replayed through mock devices
    Given the system is configured for mock device driver
    When the feature is exercised
    Then scripted input sequences can be replayed through mock devices

  Scenario: Mock device behavior is deterministic for reproducible test results
    Given the system is configured for mock device driver
    When the feature is exercised
    Then mock device behavior is deterministic for reproducible test results