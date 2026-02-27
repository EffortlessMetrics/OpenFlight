Feature: Device Virtual Pairing
  As a flight simulation enthusiast
  I want device virtual pairing
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Virtual devices can be paired with physical device inputs
    Given the system is configured for device virtual pairing
    When the feature is exercised
    Then virtual devices can be paired with physical device inputs

  Scenario: Pairing maps physical axis outputs to virtual device channels
    Given the system is configured for device virtual pairing
    When the feature is exercised
    Then pairing maps physical axis outputs to virtual device channels

  Scenario: Paired devices appear as a single logical device in profiles
    Given the system is configured for device virtual pairing
    When the feature is exercised
    Then paired devices appear as a single logical device in profiles

  Scenario: Unpairing restores both devices to independent operation
    Given the system is configured for device virtual pairing
    When the feature is exercised
    Then unpairing restores both devices to independent operation
