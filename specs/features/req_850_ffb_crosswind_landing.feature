Feature: FFB Crosswind Landing
  As a flight simulation enthusiast
  I want ffb crosswind landing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Simulate crosswind forces on approach and landing
    Given the system is configured for ffb crosswind landing
    When the feature is exercised
    Then fFB simulates crosswind forces during approach and landing phases

  Scenario: Force matches reported wind vector
    Given the system is configured for ffb crosswind landing
    When the feature is exercised
    Then force direction and magnitude correspond to reported wind vector

  Scenario: Activate below configurable altitude
    Given the system is configured for ffb crosswind landing
    When the feature is exercised
    Then effect activates below a configurable altitude threshold

  Scenario: Respect FFB safety envelope limits
    Given the system is configured for ffb crosswind landing
    When the feature is exercised
    Then crosswind forces respect FFB safety envelope limits
