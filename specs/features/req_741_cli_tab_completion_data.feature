Feature: CLI Tab Completion Data
  As a flight simulation enthusiast
  I want the CLI to generate completion data from runtime state
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Completions from runtime state
    Given the service is running with connected devices
    When I run the completions generate command
    Then completion data includes runtime state

  Scenario: Device and profile names included
    Given devices and profiles are available
    When completion data is generated
    Then device names and profile names are included

  Scenario: Scripts for bash, zsh, and fish
    Given a shell type is specified
    When completion scripts are generated
    Then output is compatible with bash, zsh, or fish

  Scenario: Completions refresh on state change
    Given the service state changes
    When completion data is regenerated
    Then it reflects the current state
