Feature: CLI Update Channel Switch
  As a flight simulation enthusiast
  I want cli update channel switch
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Switch between stable, beta, canary channels
    Given the system is configured for cli update channel switch
    When the feature is exercised
    Then cLI switches between update channels (stable, beta, canary)

  Scenario: Persist channel to config file
    Given the system is configured for cli update channel switch
    When the feature is exercised
    Then channel switch persists to the service configuration file

  Scenario: Trigger update check on switch
    Given the system is configured for cli update channel switch
    When the feature is exercised
    Then switching channels triggers an immediate update check

  Scenario: Display channel in version output
    Given the system is configured for cli update channel switch
    When the feature is exercised
    Then current channel is displayed in the CLI version output
