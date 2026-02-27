Feature: X-Plane Failure Injection
  As a flight simulation enthusiast
  I want the X-Plane adapter to support injecting failures for training
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: System failures injectable
    Given the X-Plane adapter is connected
    When a failure injection is requested
    Then the specified system failure is injected

  Scenario: Failures from button bindings
    Given a profile maps a button to a failure
    When the button is pressed
    Then the failure is triggered

  Scenario: Active failures trackable and clearable
    Given failures are currently active
    When a clear command is issued
    Then all active failures are cleared

  Scenario: Confirmation required
    Given a failure injection is requested
    When the user has not confirmed
    Then the injection is blocked until confirmed
