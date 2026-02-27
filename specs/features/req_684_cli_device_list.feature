Feature: CLI Device List Command
  As a flight simulation enthusiast
  I want the CLI to list all detected devices with their capabilities
  So that I can quickly identify connected hardware and its features

  Background:
    Given the OpenFlight service is running

  Scenario: Device list shows VID/PID and device name
    Given one or more HID devices are connected
    When I run "flightctl device list"
    Then the output includes the VID, PID, and name for each detected device

  Scenario: Device capabilities are shown
    Given one or more HID devices are connected
    When I run "flightctl device list"
    Then the output includes the capabilities of each device such as axes, buttons, and FFB support

  Scenario: JSON output is available with --json flag
    Given one or more HID devices are connected
    When I run "flightctl device list --json"
    Then the output is valid JSON containing the device list with all capability fields

  Scenario: Disconnected devices are shown as unavailable
    Given a previously known device is no longer connected
    When I run "flightctl device list"
    Then the device is listed with a status of unavailable
