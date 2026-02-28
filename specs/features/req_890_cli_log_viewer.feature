Feature: CLI Log Viewer
  As a flight simulation enthusiast
  I want cli log viewer
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Real-time log streaming displays service logs in the terminal
    Given the system is configured for cli log viewer
    When the feature is exercised
    Then real-time log streaming displays service logs in the terminal

  Scenario: Log output can be filtered by severity, source, or keyword
    Given the system is configured for cli log viewer
    When the feature is exercised
    Then log output can be filtered by severity, source, or keyword

  Scenario: Viewer supports pausing and resuming the log stream
    Given the system is configured for cli log viewer
    When the feature is exercised
    Then viewer supports pausing and resuming the log stream

  Scenario: Log viewer gracefully reconnects if the service connection drops
    Given the system is configured for cli log viewer
    When the feature is exercised
    Then log viewer gracefully reconnects if the service connection drops
