Feature: CLI Config Generate
  As a flight simulation enthusiast
  I want cli config generate
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Generate default config for new installation
    Given the system is configured for cli config generate
    When the feature is exercised
    Then cLI generates default configuration files for a new installation

  Scenario: Include documented comments in config
    Given the system is configured for cli config generate
    When the feature is exercised
    Then generated config includes documented comments for each setting

  Scenario: Protect existing files from overwrite
    Given the system is configured for cli config generate
    When the feature is exercised
    Then existing config files are not overwritten without explicit confirmation

  Scenario: Support target output directory
    Given the system is configured for cli config generate
    When the feature is exercised
    Then config generation supports targeting a specific output directory
