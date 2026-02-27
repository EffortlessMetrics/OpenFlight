Feature: Device Button Debounce Tuning
  As a flight simulation enthusiast
  I want device button debounce tuning
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Debounce timing is configurable per button on each device
    Given the system is configured for device button debounce tuning
    When the feature is exercised
    Then debounce timing is configurable per button on each device

  Scenario: Default debounce values are set based on device hardware profile
    Given the system is configured for device button debounce tuning
    When the feature is exercised
    Then default debounce values are set based on device hardware profile

  Scenario: Debounce tuning UI shows real-time bounce detection counts
    Given the system is configured for device button debounce tuning
    When the feature is exercised
    Then debounce tuning UI shows real-time bounce detection counts

  Scenario: Extremely short debounce values trigger a reliability warning
    Given the system is configured for device button debounce tuning
    When the feature is exercised
    Then extremely short debounce values trigger a reliability warning
