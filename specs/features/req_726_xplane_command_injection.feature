Feature: X-Plane Command Injection
  As a flight simulation enthusiast
  I want the X-Plane adapter to support injecting X-Plane commands
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Commands injected via UDP
    Given the X-Plane adapter is connected
    When a command injection is requested
    Then the command is sent to X-Plane via UDP

  Scenario: Commands mapped from bindings
    Given a profile has button-to-command mappings
    When the mapped button is pressed
    Then the corresponding X-Plane command is injected

  Scenario: Injection rate is throttled
    Given commands are being injected rapidly
    When the injection rate exceeds the throttle limit
    Then excess commands are queued or dropped

  Scenario: Unsupported commands rejected
    Given an unsupported command is requested
    When the adapter processes the command
    Then it is rejected with a descriptive error
