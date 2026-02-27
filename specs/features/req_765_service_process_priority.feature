Feature: Service Process Priority
  As a flight simulation enthusiast
  I want service process priority
  So that the system meets its design goals

  Background:
    Given the OpenFlight service is running

  Scenario: Set above-normal priority
    Given the system is configured for service process priority
    When the feature is exercised
    Then service sets os process priority to above-normal on startup

  Scenario: Configurable priority level
    Given the system is configured for service process priority
    When the feature is exercised
    Then priority level is configurable in service settings

  Scenario: MMCSS on Windows
    Given the system is configured for service process priority
    When the feature is exercised
    Then windows uses mmcss for thread priority elevation

  Scenario: Non-fatal priority failure
    Given the system is configured for service process priority
    When the feature is exercised
    Then failure to set priority is logged but does not prevent startup
