Feature: CLI Watch Mode
  As a flight simulation enthusiast
  I want CLI commands to support a watch/repeat mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Watch flag supported
    Given a CLI command supports watching
    When I run the command with --watch
    Then the command repeats at the configured interval

  Scenario: Configurable interval defaults to 1s
    Given watch mode is active
    When no interval is specified
    Then the default interval is 1 second

  Scenario: Terminal cleared between updates
    Given watch mode is active
    When the command output refreshes
    Then the terminal is cleared between updates

  Scenario: Clean exit on Ctrl+C
    Given watch mode is active
    When Ctrl+C is pressed
    Then watch mode exits cleanly
