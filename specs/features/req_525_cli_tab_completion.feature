@REQ-525 @product
Feature: CLI Tab Completion — Shell Completion Scripts for flightctl  @AC-525.1
  Scenario: flightctl generates completion scripts for bash, zsh, and fish
    Given the flightctl binary is installed
    When `flightctl completions bash` is executed
    Then the output SHALL be a valid bash completion script  @AC-525.2
  Scenario: Tab completion suggests valid axis names and device IDs
    Given a running flightd service with two connected devices
    When the user triggers tab completion for `flightctl axis show `
    Then the completion engine SHALL suggest the names of all currently registered axes  @AC-525.3
  Scenario: Completion scripts are generated at install time
    Given the OpenFlight installer runs on a supported platform
    When the installation completes
    Then completion scripts SHALL have been written to the platform shell completions directory  @AC-525.4
  Scenario: flightctl completions install installs completion scripts
    Given the flightctl binary is available on PATH
    When `flightctl completions install` is executed
    Then completion scripts SHALL be written and a success message SHALL be printed
