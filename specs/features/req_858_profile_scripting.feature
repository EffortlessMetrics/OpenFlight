Feature: Profile Scripting
  As a flight simulation enthusiast
  I want profile scripting
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Profile macros can be defined using a safe expression language
    Given the system is configured for profile scripting
    When the feature is exercised
    Then profile macros can be defined using a safe expression language

  Scenario: Scripts can reference axis values and sim variables as inputs
    Given the system is configured for profile scripting
    When the feature is exercised
    Then scripts can reference axis values and sim variables as inputs

  Scenario: Script execution is sandboxed with a configurable time budget
    Given the system is configured for profile scripting
    When the feature is exercised
    Then script execution is sandboxed with a configurable time budget

  Scenario: Syntax errors in scripts produce clear diagnostic messages
    Given the system is configured for profile scripting
    When the feature is exercised
    Then syntax errors in scripts produce clear diagnostic messages
