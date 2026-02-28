Feature: Update Channel Selection
  As a flight simulation enthusiast
  I want update channel selection
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: User can switch between stable, beta, and canary update channels
    Given the system is configured for update channel selection
    When the feature is exercised
    Then user can switch between stable, beta, and canary update channels

  Scenario: Channel selection persists across service restarts
    Given the system is configured for update channel selection
    When the feature is exercised
    Then channel selection persists across service restarts

  Scenario: Switching channels triggers an immediate update check
    Given the system is configured for update channel selection
    When the feature is exercised
    Then switching channels triggers an immediate update check

  Scenario: CLI command displays current channel and available channels
    Given the system is configured for update channel selection
    When the feature is exercised
    Then cLI command displays current channel and available channels
