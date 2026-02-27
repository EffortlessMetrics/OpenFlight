Feature: Axis Profile Script Engine
  As a flight simulation enthusiast
  I want to define custom axis transformations via scripts
  So that I can implement advanced input curves beyond built-in options

  Background:
    Given the OpenFlight service is running

  Scenario: Profile scripts can define custom axis transformation functions
    Given a profile with a custom axis transformation script
    When the profile is loaded
    Then the script-defined transformation is applied to axis input

  Scenario: Script engine is sandboxed with no file or network access
    When a script attempts to access the file system
    Then the access is denied and an error is returned to the script

  Scenario: Script execution time is bounded to 100 microseconds
    Given a script that runs for longer than 100 microseconds
    When the script is executed during an axis tick
    Then the script is terminated and an error is logged

  Scenario: Script errors are caught and logged without crashing service
    Given a profile script that throws a runtime error
    When the script executes
    Then the error is logged
    And the service continues running without crashing
