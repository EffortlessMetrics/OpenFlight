Feature: Device Calibration Export
  As a flight simulation enthusiast
  I want device calibration to be exportable and importable
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Calibration exportable to file
    Given a device is calibrated
    When I run the calibration export command
    Then the calibration data is saved to a file

  Scenario: Import on another system
    Given a calibration file from another system is available
    When I run the calibration import command
    Then the calibration is applied to the matching device

  Scenario: Import validates data
    Given a calibration file is provided
    When the import process begins
    Then the data is validated before being applied

  Scenario: Export and import via CLI
    Given a calibrated device is connected
    When the CLI calibration commands are used
    Then export and import complete successfully
