Feature: CLI Auto-Complete Context
  As a flight simulation enthusiast
  I want cli auto-complete context
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Completions are context-aware based on the current command prefix
    Given the system is configured for cli auto-complete context
    When the feature is exercised
    Then completions are context-aware based on the current command prefix

  Scenario: Device names and profile names are suggested from live service data
    Given the system is configured for cli auto-complete context
    When the feature is exercised
    Then device names and profile names are suggested from live service data

  Scenario: Completion scripts are generated for bash, zsh, fish, and PowerShell
    Given the system is configured for cli auto-complete context
    When the feature is exercised
    Then completion scripts are generated for bash, zsh, fish, and PowerShell

  Scenario: Completions update dynamically when devices connect or disconnect
    Given the system is configured for cli auto-complete context
    When the feature is exercised
    Then completions update dynamically when devices connect or disconnect
