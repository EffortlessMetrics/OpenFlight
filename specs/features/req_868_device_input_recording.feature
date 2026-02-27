Feature: Device Input Recording
  As a flight simulation enthusiast
  I want device input recording
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Raw device input streams can be recorded to a timestamped file
    Given the system is configured for device input recording
    When the feature is exercised
    Then raw device input streams can be recorded to a timestamped file

  Scenario: Recording captures all axes, buttons, and hat positions
    Given the system is configured for device input recording
    When the feature is exercised
    Then recording captures all axes, buttons, and hat positions

  Scenario: Playback of recorded input is supported for diagnostics
    Given the system is configured for device input recording
    When the feature is exercised
    Then playback of recorded input is supported for diagnostics

  Scenario: Recording automatically stops when file size limit is reached
    Given the system is configured for device input recording
    When the feature is exercised
    Then recording automatically stops when file size limit is reached
