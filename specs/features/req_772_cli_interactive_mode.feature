Feature: CLI Interactive Mode
  As a flight simulation enthusiast
  I want cli interactive mode
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Interactive REPL subcommand
    Given the system is configured for cli interactive mode
    When the feature is exercised
    Then cli supports a repl mode with the interactive subcommand

  Scenario: Tab completion support
    Given the system is configured for cli interactive mode
    When the feature is exercised
    Then repl provides tab completion for commands and arguments

  Scenario: Persistent command history
    Given the system is configured for cli interactive mode
    When the feature is exercised
    Then command history is preserved across repl sessions

  Scenario: Live status updates
    Given the system is configured for cli interactive mode
    When the feature is exercised
    Then repl displays live status updates between commands
