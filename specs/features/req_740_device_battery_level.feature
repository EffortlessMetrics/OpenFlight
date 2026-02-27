Feature: Device Battery Level
  As a flight simulation enthusiast
  I want the service to track wireless device battery levels
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Battery level tracked
    Given a wireless device with battery reporting is connected
    When the device reports battery level
    Then the service tracks the level

  Scenario: Level queryable via IPC
    Given the service is tracking battery levels
    When a client queries battery level via IPC
    Then the current level is returned

  Scenario: Low battery triggers warning
    Given a device battery level drops below threshold
    When the level is evaluated
    Then a warning notification is triggered

  Scenario: Wired devices report N/A
    Given a wired device is connected
    When battery level is queried
    Then it reports not applicable
