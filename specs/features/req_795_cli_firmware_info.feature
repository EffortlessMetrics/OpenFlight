Feature: CLI Firmware Info
  As a flight simulation enthusiast
  I want cli firmware info
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Display firmware version
    Given the system is configured for cli firmware info
    When the feature is exercised
    Then cli displays device firmware version in device info output

  Scenario: Read from HID descriptor
    Given the system is configured for cli firmware info
    When the feature is exercised
    Then firmware version is read from the hid device descriptor

  Scenario: Placeholder for unknown version
    Given the system is configured for cli firmware info
    When the feature is exercised
    Then unknown firmware version displays a placeholder

  Scenario: Include in diagnostic export
    Given the system is configured for cli firmware info
    When the feature is exercised
    Then firmware info is included in diagnostic export
