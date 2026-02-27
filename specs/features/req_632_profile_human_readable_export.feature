Feature: Profile Export to Human-Readable Format
  As a pilot using OpenFlight
  I want to export profiles in a human-readable diff format
  So that I can review configuration changes easily

  Background:
    Given the OpenFlight service is running

  Scenario: flightctl profile diff shows axis-by-axis configuration summary
    Given a profile is loaded
    When flightctl profile diff is run
    Then the output shows a per-axis configuration summary

  Scenario: Diff output is colorized when terminal supports it
    Given the terminal reports color support
    When flightctl profile diff is run
    Then the diff output includes ANSI color codes

  Scenario: Diff supports comparison between two profile files
    Given two profile files exist on disk
    When flightctl profile diff is run with both files as arguments
    Then the output shows differences between the two profiles

  Scenario: Diff output is parseable in script mode
    When flightctl --script profile diff is run
    Then the output is machine-parseable with no color or progress indicators
