Feature: Axis Deadzone Preview in CLI
  As a flight simulation enthusiast
  I want the CLI to show a live preview of axis deadzone
  So that I can visually verify deadzone configuration without a GUI

  Background:
    Given the OpenFlight service is running and a HID device is connected

  Scenario: flightctl axis monitor shows current position and deadzone region
    When the command "flightctl axis monitor" is run for a configured axis
    Then the output shows the current axis position and the active deadzone region

  Scenario: Position is shown as ASCII bar chart
    When the axis monitor is active
    Then the current position is displayed as an ASCII bar chart in the terminal

  Scenario: Deadzone boundary markers are visible in the chart
    Given the axis has a non-zero deadzone configured
    When the axis monitor renders the bar chart
    Then the deadzone boundary positions are marked with visible characters in the chart

  Scenario: Update rate is configurable
    Given the axis monitor is started with "--rate 10"
    Then the display refreshes at approximately 10 times per second
